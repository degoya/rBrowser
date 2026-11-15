//! The agent in the device's page (RB-080-05): an HTML answer gets one `<script>` that loads the
//! portal agent from the portal itself (`/__rbrowser/agent.js`, so a `script-src 'self'` policy
//! admits it), right after `<head>` - or after `<html>`, or first. The body is read whole for
//! that, up to [`MAX_HTML`]; a larger or compressed page passes unchanged, without the agent.

/// Where the portal serves its agent.
pub const AGENT_PATH: &str = "/__rbrowser/agent.js";
/// The tag put into every HTML page.
pub const AGENT_TAG: &str = "<script src=\"/__rbrowser/agent.js\" defer></script>";
/// The largest HTML body the portal rewrites.
pub const MAX_HTML: usize = 8 * 1024 * 1024;

/// `html` with [`AGENT_TAG`] in it.
#[must_use]
pub fn inject(html: &[u8]) -> Vec<u8> {
    let at = tag_end(html, b"<head")
        .or_else(|| tag_end(html, b"<html"))
        .unwrap_or(0);
    let mut out = Vec::with_capacity(html.len() + AGENT_TAG.len());
    out.extend_from_slice(&html[..at]);
    out.extend_from_slice(AGENT_TAG.as_bytes());
    out.extend_from_slice(&html[at..]);
    out
}

/// `html` with every absolute URL of the upstream origin `from` pointed at the portal origin
/// `to`: a site that writes its own origin into links and asset URLs (`APP_URL`) would send the
/// device to a host it cannot reach. The origin matches in any case and only whole (`:3000`,
/// not `:30001`); anything else, a `https` twin included, stays.
#[must_use]
pub fn rewrite_html(html: &[u8], from: &str, to: &str) -> Vec<u8> {
    let lower = html.to_ascii_lowercase();
    let from = from.to_ascii_lowercase();
    let mut out = Vec::with_capacity(html.len());
    let mut at = 0;
    while let Some(found) = find(&lower[at..], from.as_bytes()) {
        let start = at + found;
        let end = start + from.len();
        let whole = match lower.get(end) {
            None => true,
            Some(next) => {
                !next.is_ascii_alphanumeric() && !matches!(next, b'.' | b'-' | b'_' | b':')
            }
        };
        if whole {
            out.extend_from_slice(&html[at..start]);
            out.extend_from_slice(to.as_bytes());
        } else {
            out.extend_from_slice(&html[at..end]);
        }
        at = end;
    }
    out.extend_from_slice(&html[at..]);
    out
}

/// The offset after the `>` of the first tag `name` (`<head`, not `<header`), in any case.
fn tag_end(html: &[u8], name: &[u8]) -> Option<usize> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = find(&lower[from..], name) {
        let start = from + found;
        let after = start + name.len();
        match lower.get(after) {
            Some(b'>' | b' ' | b'\t' | b'\r' | b'\n' | b'/') => {
                return find(&lower[after..], b">").map(|close| after + close + 1);
            }
            _ => from = after,
        }
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The length of a complete body in `bytes`: `length` bytes, or the chunks up to the last one;
/// `None` while more is to come (or, with neither, until the upstream closes).
#[must_use]
pub fn complete(bytes: &[u8], length: Option<usize>, chunked: bool) -> Option<usize> {
    if chunked {
        return chunked_end(bytes);
    }
    length.filter(|length| bytes.len() >= *length)
}

/// The decoded body of complete chunked `bytes`.
#[must_use]
pub fn decode_chunked(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    walk_chunks(bytes, |chunk| out.extend_from_slice(chunk)).map(|_| out)
}

fn chunked_end(bytes: &[u8]) -> Option<usize> {
    walk_chunks(bytes, |_| {})
}

/// Walks the chunks of `bytes`, handing each to `each`; the offset after the last chunk's
/// trailer, or `None` for an incomplete or broken body.
fn walk_chunks(bytes: &[u8], mut each: impl FnMut(&[u8])) -> Option<usize> {
    let mut at = 0;
    loop {
        let line_end = at + find(bytes.get(at..)?, b"\r\n")?;
        let line = std::str::from_utf8(&bytes[at..line_end]).ok()?;
        let size_text = line.split(';').next()?.trim();
        let size = usize::from_str_radix(size_text, 16).ok()?;
        at = line_end + 2;
        if size == 0 {
            // Trailers up to the blank line.
            let rest = bytes.get(at..)?;
            if rest.starts_with(b"\r\n") {
                return Some(at + 2);
            }
            return find(rest, b"\r\n\r\n").map(|end| at + end + 4);
        }
        let chunk = bytes.get(at..at.checked_add(size)?)?;
        each(chunk);
        at += size;
        if bytes.get(at..at + 2)? != b"\r\n" {
            return None;
        }
        at += 2;
    }
}

#[cfg(test)]
#[path = "inject_tests.rs"]
mod tests;
