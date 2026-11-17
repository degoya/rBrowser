//! A text body rewritten as it streams (RB-090-72): a style sheet, a script or JSON - and an
//! HTML page that takes no agent - names the page's own upstream origin (an API payload that
//! names its images, a bundle that builds URLs) whether or not asset origins are mapped, and a
//! streamed answer (chunked, a long poll, an event stream) or a large bundle must not be held
//! back until it ends. [`BodyReader`] takes the transfer coding off as the bytes come,
//! [`StreamRewrite`] replaces each upstream origin through the [`OriginMap`] - plain, with
//! escaped slashes or as character references (RB-090-75), also when a read cuts one in two -
//! keeping back only what may still become an origin, and [`stream_rewritten`] sends the result chunked anew (`no-store`, without the
//! upstream's validators, [`OutBody::Chunked`]). The body scan for unmapped loopback origins
//! runs on it as on a body read whole.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::origin_map::{Needles, OriginMap, body_origins, report_unmapped, report_unrewritten};
use super::rewrite::{OutBody, ResponseHead, response_head};
use super::url_forms::{authority_at, could_be_scheme, scheme_at};

/// The longest chunk-size line (with its extensions) the portal reads.
pub const MAX_SIZE_LINE: usize = 1024;
/// The longest trailer section the portal reads (and drops).
const MAX_TRAILERS: usize = 8 * 1024;
/// The longest URL start kept back while its host may still go on, for the body scan.
pub const MAX_OPEN_URL: usize = 256;
/// The most unmapped loopback origins one streamed body reports.
const MAX_FOUND: usize = 16;
/// The end of a chunked body.
pub const LAST_CHUNK: &[u8] = b"0\r\n\r\n";

/// A body whose transfer coding is broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the upstream's chunked body is malformed")]
pub struct BodyError;

/// Where a chunked body stands.
#[derive(Debug)]
enum Chunked {
    /// Reading a size line.
    Size(Vec<u8>),
    /// This many bytes of the chunk to come.
    Data(usize),
    /// This many bytes of the CRLF after a chunk seen.
    DataEnd(usize),
    /// Reading the trailers after the last chunk.
    Trailers(Vec<u8>),
    /// The body ended.
    Done,
}

/// How the body ends.
#[derive(Debug)]
enum Framing {
    /// After this many more bytes.
    Length(usize),
    /// With its last chunk.
    Chunked(Chunked),
    /// When the upstream closes.
    Close,
}

/// A response body read as it comes: its payload without the transfer coding.
#[derive(Debug)]
pub struct BodyReader {
    framing: Framing,
}

impl BodyReader {
    /// The reader of a body with `length` (`Content-Length`) or `chunked`; chunked wins.
    #[must_use]
    pub fn new(length: Option<usize>, chunked: bool) -> Self {
        let framing = match (chunked, length) {
            (true, _) => Framing::Chunked(Chunked::Size(Vec::new())),
            (false, Some(length)) => Framing::Length(length),
            (false, None) => Framing::Close,
        };
        Self { framing }
    }

    /// The payload in `input`; what comes after the body's end is dropped.
    ///
    /// # Errors
    ///
    /// [`BodyError`] for a malformed chunked body.
    pub fn feed(&mut self, input: &[u8]) -> Result<Vec<u8>, BodyError> {
        match &mut self.framing {
            Framing::Length(left) => {
                let take = (*left).min(input.len());
                *left -= take;
                Ok(input[..take].to_vec())
            }
            Framing::Close => Ok(input.to_vec()),
            Framing::Chunked(state) => {
                let mut out = Vec::with_capacity(input.len());
                feed_chunked(state, input, &mut out)?;
                Ok(out)
            }
        }
    }

    /// Whether the body ended.
    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(
            self.framing,
            Framing::Length(0) | Framing::Chunked(Chunked::Done)
        )
    }

    /// Whether the body is whole when the upstream closes now.
    #[must_use]
    pub fn complete_at_close(&self) -> bool {
        matches!(self.framing, Framing::Close) || self.is_done()
    }
}

/// Feeds `input` to the chunked body at `state`, the payload into `out`.
fn feed_chunked(state: &mut Chunked, mut input: &[u8], out: &mut Vec<u8>) -> Result<(), BodyError> {
    while !input.is_empty() {
        match state {
            Chunked::Size(line) => {
                let Some(end) = input.iter().position(|byte| *byte == b'\n') else {
                    line.extend_from_slice(input);
                    if line.len() > MAX_SIZE_LINE {
                        return Err(BodyError);
                    }
                    return Ok(());
                };
                line.extend_from_slice(&input[..end]);
                input = &input[end + 1..];
                if line.len() > MAX_SIZE_LINE {
                    return Err(BodyError);
                }
                let text = std::str::from_utf8(line).map_err(|_| BodyError)?;
                let size_text = text.trim_end_matches('\r').split(';').next().unwrap_or("");
                let size_text = size_text.trim();
                if size_text.is_empty() || !size_text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(BodyError);
                }
                let size = usize::from_str_radix(size_text, 16).map_err(|_| BodyError)?;
                *state = if size == 0 {
                    Chunked::Trailers(Vec::new())
                } else {
                    Chunked::Data(size)
                };
            }
            Chunked::Data(left) => {
                let take = (*left).min(input.len());
                out.extend_from_slice(&input[..take]);
                input = &input[take..];
                *left -= take;
                if *left == 0 {
                    *state = Chunked::DataEnd(0);
                }
            }
            Chunked::DataEnd(seen) => {
                if input[0] != b"\r\n"[*seen] {
                    return Err(BodyError);
                }
                input = &input[1..];
                *seen += 1;
                if *seen == 2 {
                    *state = Chunked::Size(Vec::new());
                }
            }
            Chunked::Trailers(section) => {
                section.push(input[0]);
                input = &input[1..];
                if section.as_slice() == b"\r\n" || section.ends_with(b"\r\n\r\n") {
                    *state = Chunked::Done;
                } else if section.len() > MAX_TRAILERS {
                    return Err(BodyError);
                }
            }
            Chunked::Done => return Ok(()),
        }
    }
    Ok(())
}

/// `bytes` as one chunk; nothing for no bytes (an empty chunk would end the body).
#[must_use]
pub fn chunk(bytes: &[u8]) -> Vec<u8> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let mut out = format!("{:x}\r\n", bytes.len()).into_bytes();
    out.extend_from_slice(bytes);
    out.extend_from_slice(b"\r\n");
    out
}

/// Whether `rest` (lower case) is the start of a URL whose host may still go on, or of its
/// scheme (`http:/`, `http&#x3a;&#x2f;`), in any form: kept back so the scan reads the host whole.
fn open_url(rest: &[u8]) -> bool {
    rest.len() <= MAX_OPEN_URL
        && (could_be_scheme(rest)
            || scheme_at(rest, 0).is_some_and(|scheme| authority_at(rest, scheme).open))
}

/// A body rewritten piece by piece: each piece's output is what can no longer change.
#[derive(Debug)]
pub struct StreamRewrite {
    map: OriginMap,
    needles: Needles,
    carry: Vec<u8>,
    found: Vec<String>,
    unrewritten: Vec<String>,
}

impl StreamRewrite {
    /// A stream rewritten through `map`.
    #[must_use]
    pub fn new(map: &OriginMap) -> Self {
        Self {
            map: map.clone(),
            needles: map.needles(),
            carry: Vec::new(),
            found: Vec::new(),
            unrewritten: Vec::new(),
        }
    }

    /// The output for `input`, the bytes after what came before.
    pub fn push(&mut self, input: &[u8]) -> Vec<u8> {
        let mut buffer = std::mem::take(&mut self.carry);
        buffer.extend_from_slice(input);
        // Every origin that starts before `cut` can be told whole; after it, only what may still
        // become one (or a URL whose host goes on) waits for more.
        let cut = buffer
            .len()
            .saturating_sub(self.needles.reach().max(MAX_OPEN_URL));
        // Every origin and every scheme form starts with `http`: only an `h` can start what
        // waits, so the forms never multiply the positions tried.
        let lower = buffer[cut..].to_ascii_lowercase();
        let keep = (0..lower.len())
            .filter(|&at| lower[at] == b'h')
            .find(|&at| self.needles.could_start(&lower[at..]) || open_url(&lower[at..]))
            .map_or(buffer.len(), |at| cut + at);
        let (out, consumed) = self.needles.rewrite(&buffer, keep);
        self.scan(&buffer, keep, false);
        self.carry = buffer.split_off(consumed);
        out
    }

    /// The output for what was kept back: the body ended.
    pub fn finish(&mut self) -> Vec<u8> {
        let buffer = std::mem::take(&mut self.carry);
        self.scan(&buffer, buffer.len(), true);
        self.needles.rewrite(&buffer, buffer.len()).0
    }

    /// How many bytes wait for more.
    #[must_use]
    pub fn held(&self) -> usize {
        self.carry.len()
    }

    /// The unmapped loopback origins found so far.
    #[must_use]
    pub fn found(&self) -> &[String] {
        &self.found
    }

    /// The mapped origins found so far in a form the rewrite does not match (RB-090-75 review).
    #[must_use]
    pub fn unrewritten(&self) -> &[String] {
        &self.unrewritten
    }

    /// Reports the unmapped loopback origins and the rewrite gaps named before `cut` that are new
    /// to this body.
    fn scan(&mut self, buffer: &[u8], cut: usize, at_end: bool) {
        if self.found.len() >= MAX_FOUND && self.unrewritten.len() >= MAX_FOUND {
            return;
        }
        let origins = body_origins(buffer, &self.map, cut, at_end);
        let unmapped = fresh(&mut self.found, origins.unmapped);
        if !unmapped.is_empty() {
            report_unmapped(&unmapped, &self.map);
        }
        report_unrewritten(&fresh(&mut self.unrewritten, origins.unrewritten));
    }
}

/// The origins of `found` that `known` lacks, as many as fit under [`MAX_FOUND`]; added to it.
fn fresh(known: &mut Vec<String>, found: Vec<String>) -> Vec<String> {
    let new: Vec<String> = found
        .into_iter()
        .filter(|origin| !known.contains(origin))
        .take(MAX_FOUND.saturating_sub(known.len()))
        .collect();
    known.extend(new.iter().cloned());
    new
}

/// Sends `head` and its body from `server` (`first`: the body bytes read with the head) to
/// `client`, rewritten through `map` and chunked anew. An upstream that closes before the body
/// ends leaves the device's body without its last chunk, so the device sees it cut short.
///
/// # Errors
///
/// A socket failed, or the upstream's chunked body is malformed.
pub async fn stream_rewritten(
    server: &mut (impl AsyncRead + Unpin),
    client: &mut (impl AsyncWrite + Unpin),
    head: &ResponseHead,
    map: &OriginMap,
    first: &[u8],
) -> std::io::Result<()> {
    let out = response_head(head, map, OutBody::Chunked);
    client.write_all(out.as_bytes()).await?;
    let mut reader = BodyReader::new(head.content_length(), head.is_chunked());
    let mut rewrite = StreamRewrite::new(map);
    let mut buffer = vec![0u8; 16 * 1024];
    let mut input = first.to_vec();
    loop {
        let payload = reader
            .feed(&input)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        client.write_all(&chunk(&rewrite.push(&payload))).await?;
        if reader.is_done() {
            break;
        }
        let read = server.read(&mut buffer).await?;
        if read == 0 {
            if !reader.complete_at_close() {
                return Ok(());
            }
            break;
        }
        input.clear();
        input.extend_from_slice(&buffer[..read]);
    }
    client.write_all(&chunk(&rewrite.finish())).await?;
    client.write_all(LAST_CHUNK).await
}

#[cfg(test)]
#[path = "stream_body_tests.rs"]
mod tests;
