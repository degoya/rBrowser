//! The HTTP request head of a proxy connection: read up to the blank line (at most
//! [`MAX_HEAD`] bytes), parsed without a library, and the WebSocket upgrade it may ask for
//! (RFC 6455: `GET`, `Upgrade: websocket`, `Connection: upgrade`, version 13, a key). The proxy
//! answers `/json/*` itself and hands an upgrade to the WebSocket layer.

/// The longest request head read; a longer one is refused (431).
pub const MAX_HEAD: usize = 8 * 1024;

/// A parsed request head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestHead {
    /// `GET`.
    pub method: String,
    /// The request target as sent: path and query.
    pub target: String,
    /// The headers, names as sent, values trimmed.
    pub headers: Vec<(String, String)>,
}

impl RequestHead {
    /// The first header named `name`, in any case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The target without its query.
    #[must_use]
    pub fn path(&self) -> &str {
        self.target
            .split_once('?')
            .map_or(self.target.as_str(), |(path, _)| path)
    }
}

/// Why a head was not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadError {
    /// Longer than [`MAX_HEAD`].
    TooLarge,
    /// No HTTP/1.x request head.
    Malformed,
}

/// The length of the head in `buffer` up to and including its blank line, once it is there.
#[must_use]
pub fn head_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|start| start + 4)
}

/// The head in `bytes` (up to its blank line; anything after is ignored).
///
/// # Errors
///
/// [`HeadError::TooLarge`] past [`MAX_HEAD`], [`HeadError::Malformed`] for anything that is no
/// HTTP/1.x request head with an absolute path.
pub fn parse_head(bytes: &[u8]) -> Result<RequestHead, HeadError> {
    let end = head_end(bytes).unwrap_or(bytes.len());
    if end > MAX_HEAD {
        return Err(HeadError::TooLarge);
    }
    let text = std::str::from_utf8(&bytes[..end]).map_err(|_| HeadError::Malformed)?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().ok_or(HeadError::Malformed)?;
    let parts: Vec<&str> = request_line.split(' ').collect();
    let [method, target, version] = parts.as_slice() else {
        return Err(HeadError::Malformed);
    };
    let token = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_uppercase());
    if !token(method) || !target.starts_with('/') || !version.starts_with("HTTP/1.") {
        return Err(HeadError::Malformed);
    }
    let mut headers = Vec::new();
    for line in lines.take_while(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or(HeadError::Malformed)?;
        if name.is_empty() || name.contains(char::is_whitespace) {
            return Err(HeadError::Malformed);
        }
        headers.push((name.to_owned(), value.trim().to_owned()));
    }
    Ok(RequestHead {
        method: (*method).to_owned(),
        target: (*target).to_owned(),
        headers,
    })
}

/// The `Sec-WebSocket-Key` of a complete WebSocket upgrade request; `None` for anything else.
#[must_use]
pub fn websocket_key(head: &RequestHead) -> Option<&str> {
    let has_token = |name: &str, token: &str| {
        head.header(name).is_some_and(|value| {
            value
                .split(',')
                .any(|part| part.trim().eq_ignore_ascii_case(token))
        })
    };
    let upgrade = head.method == "GET"
        && has_token("upgrade", "websocket")
        && has_token("connection", "upgrade")
        && head.header("sec-websocket-version") == Some("13");
    head.header("sec-websocket-key")
        .filter(|key| upgrade && !key.is_empty())
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
