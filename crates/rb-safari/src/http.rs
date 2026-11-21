//! A lean HTTP/1.1 client for `safaridriver`'s W3C WebDriver endpoint: one request per
//! connection (`Connection: close`), JSON in and out, only ever to `127.0.0.1` on the driver's
//! port - the address is built here from the port, never parsed from anything the driver or a
//! page says, and every path segment taken from an answer must be plain ([`is_plain_segment`]).
//! An answer ends at its `Content-Length`, the last chunk of a `chunked` body, or the end of the
//! connection, and is capped in size. No body is ever logged or quoted in an error: a replayed
//! password travels in them.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// The largest answer read: a screenshot of a long page, base64.
pub const MAX_ANSWER_BYTES: usize = 256 * 1024 * 1024;
/// The longest path segment taken from an answer (a session or element id).
const MAX_SEGMENT: usize = 128;
/// The longest line of a `chunked` body's framing: a chunk's size with its extensions, or a
/// trailer.
const MAX_CHUNK_LINE: usize = 4096;

/// The HTTP methods WebDriver uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `DELETE`.
    Delete,
}

impl Method {
    /// The method's name on the request line.
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Delete => "DELETE",
        }
    }
}

/// An answer: the status and the JSON body (`Null` when empty).
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// The HTTP status.
    pub status: u16,
    /// The body.
    pub body: Value,
}

/// Why no answer was understood. Never quotes what came in.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HttpError {
    /// Nobody listens on the port (the driver ended or never started).
    #[error("cannot connect: {0}")]
    Connect(String),
    /// The connection broke.
    #[error("the connection broke: {0}")]
    Io(String),
    /// No answer in time.
    #[error("no answer within {0} ms")]
    Timeout(u64),
    /// Not HTTP, not JSON, or too large.
    #[error("{0}")]
    Malformed(String),
}

/// The loopback address of `port`: the only address this client dials.
pub fn loopback(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

/// Whether `segment` may go into a request path: letters, digits, `-`, `_` and `.` (not `..`),
/// at most 128 of them - what session and element ids are.
pub fn is_plain_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= MAX_SEGMENT
        && !segment.contains("..")
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// The bytes of one request: `path` on the driver's `port`, with `body` as JSON.
pub fn request_bytes(method: Method, port: u16, path: &str, body: Option<&Value>) -> Vec<u8> {
    let payload = body.map(Value::to_string).unwrap_or_default();
    let mut head = format!(
        "{} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: application/json\r\nConnection: close\r\n",
        method.as_str()
    );
    if body.is_some() {
        head.push_str("Content-Type: application/json; charset=utf-8\r\n");
    }
    head.push_str(&format!("Content-Length: {}\r\n\r\n", payload.len()));
    let mut bytes = head.into_bytes();
    bytes.extend_from_slice(payload.as_bytes());
    bytes
}

/// Sends one request to `127.0.0.1:port` and reads its answer, all within `timeout`.
pub async fn request(
    port: u16,
    method: Method,
    path: &str,
    body: Option<&Value>,
    timeout: Duration,
) -> Result<Answer, HttpError> {
    let exchange = async {
        let mut stream = TcpStream::connect(loopback(port))
            .await
            .map_err(|error| HttpError::Connect(error.kind().to_string()))?;
        stream
            .write_all(&request_bytes(method, port, path, body))
            .await
            .map_err(io)?;
        read_answer(&mut stream).await
    };
    match tokio::time::timeout(timeout, exchange).await {
        Ok(result) => result,
        Err(_) => Err(HttpError::Timeout(
            u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
        )),
    }
}

async fn read_answer(stream: &mut TcpStream) -> Result<Answer, HttpError> {
    let mut parser = AnswerParser::default();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = stream.read(&mut buffer).await.map_err(io)?;
        if read == 0 && parser.is_empty() {
            // Closed before a byte came: the driver is going away.
            return Err(HttpError::Io("closed without an answer".to_owned()));
        }
        if parser.len().saturating_add(read) > MAX_ANSWER_BYTES {
            return Err(malformed("the answer is too large"));
        }
        if let Some(answer) = parser.push(buffer.get(..read).unwrap_or_default(), read == 0)? {
            return Ok(answer);
        }
        if read == 0 {
            return Err(malformed("the answer ended early"));
        }
    }
}

fn io(error: std::io::Error) -> HttpError {
    HttpError::Io(error.kind().to_string())
}

fn malformed(reason: &str) -> HttpError {
    HttpError::Malformed(reason.to_owned())
}

/// How the body of an answer ends.
enum Framing {
    Length(usize),
    Chunked(Chunks),
    ToEnd,
    Empty,
}

/// The parsed header: the status, the framing and where the body starts.
struct Head {
    status: u16,
    framing: Framing,
    body_start: usize,
}

/// An answer read piece by piece: the header is parsed once, a `chunked` body joined as its
/// chunks come, so every byte is looked at about once however the answer arrives.
#[derive(Default)]
pub struct AnswerParser {
    raw: Vec<u8>,
    /// Where the search for the header's end goes on.
    scanned: usize,
    head: Option<Head>,
}

impl AnswerParser {
    /// The bytes taken so far.
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    /// Whether no byte came yet.
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// Takes `bytes`; the answer once it is complete, `None` while more is to come (`eof`:
    /// nothing more comes, so an incomplete answer is an error).
    pub fn push(&mut self, bytes: &[u8], eof: bool) -> Result<Option<Answer>, HttpError> {
        self.raw.extend_from_slice(bytes);
        if self.head.is_none() {
            let unseen = self.raw.get(self.scanned..).unwrap_or_default();
            let Some(at) = find(unseen, b"\r\n\r\n") else {
                // The end may straddle this piece and the next.
                self.scanned = self.raw.len().saturating_sub(3);
                return if eof {
                    Err(malformed("the answer ended inside its header"))
                } else {
                    Ok(None)
                };
            };
            let end = self.scanned + at;
            let (status, framing) = parse_head(self.raw.get(..end).unwrap_or_default())?;
            self.head = Some(Head {
                status,
                framing,
                body_start: end + 4,
            });
        }
        let Some(head) = self.head.as_mut() else {
            return Ok(None);
        };
        let rest = self.raw.get(head.body_start..).unwrap_or_default();
        let body = match &mut head.framing {
            Framing::Empty => Vec::new(),
            Framing::Length(length) => match rest.get(..*length) {
                Some(body) => body.to_vec(),
                None if eof => return Err(malformed("the answer is shorter than its length")),
                None => return Ok(None),
            },
            Framing::Chunked(chunks) => {
                if chunks.feed(rest)? {
                    std::mem::take(&mut chunks.body)
                } else if eof {
                    return Err(malformed("the answer ended inside a chunk"));
                } else {
                    return Ok(None);
                }
            }
            Framing::ToEnd if eof => rest.to_vec(),
            Framing::ToEnd => return Ok(None),
        };
        let body = if body.iter().all(u8::is_ascii_whitespace) {
            Value::Null
        } else {
            serde_json::from_slice(&body).map_err(|_| malformed("the answer is not JSON"))?
        };
        Ok(Some(Answer {
            status: head.status,
            body,
        }))
    }
}

/// The answer in `raw` once it is complete; `None` while more is to come (`eof`: nothing more
/// comes, so an incomplete answer is an error).
pub fn try_parse(raw: &[u8], eof: bool) -> Result<Option<Answer>, HttpError> {
    AnswerParser::default().push(raw, eof)
}

fn parse_head(head: &[u8]) -> Result<(u16, Framing), HttpError> {
    let head = std::str::from_utf8(head).map_err(|_| malformed("the header is not text"))?;
    let mut lines = head.split("\r\n");
    let mut status_line = lines.next().unwrap_or_default().split(' ');
    if !status_line
        .next()
        .is_some_and(|version| version.starts_with("HTTP/1."))
    {
        return Err(malformed("the answer is not HTTP/1"));
    }
    let status: u16 = status_line
        .next()
        .and_then(|status| status.parse().ok())
        .ok_or_else(|| malformed("the answer has no status"))?;
    let mut framing = Framing::ToEnd;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("transfer-encoding")
            && value.to_ascii_lowercase().contains("chunked")
        {
            framing = Framing::Chunked(Chunks::default());
        } else if name.eq_ignore_ascii_case("content-length")
            && !matches!(framing, Framing::Chunked(_))
        {
            let length = value
                .parse()
                .map_err(|_| malformed("the answer's length is not a number"))?;
            framing = Framing::Length(length);
        }
    }
    if matches!(status, 204 | 304) || (100..200).contains(&status) {
        framing = Framing::Empty;
    }
    Ok((status, framing))
}

/// A `chunked` body joined as its chunks come.
#[derive(Default)]
struct Chunks {
    /// How far into the body the chunks were taken.
    taken: usize,
    /// The last chunk came; what follows are trailers up to an empty line.
    last: bool,
    body: Vec<u8>,
}

impl Chunks {
    /// Takes the complete chunks of `rest` (the whole body so far); whether the body ended.
    fn feed(&mut self, rest: &[u8]) -> Result<bool, HttpError> {
        loop {
            let unseen = rest.get(self.taken..).unwrap_or_default();
            let Some(line) = chunk_line(unseen)? else {
                return Ok(false);
            };
            let after_line = line.len() + 2;
            if self.last {
                self.taken += after_line;
                if line.is_empty() {
                    return Ok(true);
                }
                continue;
            }
            let size_text = std::str::from_utf8(line)
                .map_err(|_| malformed("a chunk's size is not text"))?
                .split(';')
                .next()
                .unwrap_or_default()
                .trim();
            let size = usize::from_str_radix(size_text, 16)
                .ok()
                .filter(|size| *size <= MAX_ANSWER_BYTES)
                .ok_or_else(|| malformed("a chunk's size is not a hex number in range"))?;
            if size == 0 {
                self.last = true;
                self.taken += after_line;
                continue;
            }
            let data_end = after_line
                .checked_add(size)
                .ok_or_else(|| malformed("a chunk's size is out of range"))?;
            let chunk_end = data_end
                .checked_add(2)
                .ok_or_else(|| malformed("a chunk's size is out of range"))?;
            let Some(chunk) = unseen.get(..chunk_end) else {
                return Ok(false);
            };
            if chunk.get(data_end..) != Some(b"\r\n".as_slice()) {
                return Err(malformed("a chunk does not end its line"));
            }
            self.body
                .extend_from_slice(chunk.get(after_line..data_end).unwrap_or_default());
            self.taken += chunk_end;
        }
    }
}

/// The first line of `unseen` without its `\r\n`; `None` while it has not ended, an error once
/// it is longer than any framing line may be.
fn chunk_line(unseen: &[u8]) -> Result<Option<&[u8]>, HttpError> {
    let window = unseen
        .get(..unseen.len().min(MAX_CHUNK_LINE + 2))
        .unwrap_or_default();
    match find(window, b"\r\n") {
        Some(end) => Ok(window.get(..end)),
        None if unseen.len() >= MAX_CHUNK_LINE + 2 => Err(malformed("a chunk's line is too long")),
        None => Ok(None),
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
