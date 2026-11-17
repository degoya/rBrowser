//! Forwarding one device request to the upstream (RB-080-05): one upstream connection per
//! request (`Connection: close` both ways), the heads rewritten by [`super::rewrite`]. An
//! uncompressed `200` HTML page of the page's own upstream is read whole (up to [`MAX_HTML`]),
//! its origins rewritten through the [`OriginMap`] and the portal agent put in
//! ([`super::inject`]). Every other uncompressed `200` text body - HTML from an asset listener, a
//! style sheet, a script, JSON - is rewritten as it streams ([`super::stream_body`], RB-090-72),
//! with or without asset origins: it may name the page's own upstream (an API payload naming its
//! images, RB-090-69 its escaped form; Vite's `server.origin`, RB-090-65), and a long poll or a
//! large bundle is never held back; so is an HTML page too large to read whole. Every rewritten
//! body is searched for loopback origins the map does not forward, logged once each as
//! evidence. Every other answer streams through as it comes. A WebSocket upgrade (Vite's HMR) is passed through both ways once the
//! upstream switched protocols. Every forwarded connection - a long poll, an event stream, an
//! upgrade - ends when its device is disconnected or the portal stops (Charge 10 review M1).

use std::future::Future;
use std::time::Duration;

use rb_cdp::request::{HeadError, MAX_HEAD, RequestHead, head_end};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

use super::inject::{MAX_HTML, complete, decode_chunked, inject};
use super::origin_map::{OriginMap, log_unmapped_loopback};
use super::origins::Upstream;
use super::rewrite::{OutBody, ResponseHead, parse_response_head, request_head, response_head};
use super::stream_body::stream_rewritten;

/// How long the upstream may take to accept a connection and to send its answer's head.
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(30);

/// Forwards `request` (with the body bytes already read, `rest`) to `upstream` and answers
/// the device on `client`; `map` holds the portal's origins for the upstream ones. When `ended` completes (the device
/// was disconnected, the portal stopped) both connections close at once.
///
/// # Errors
///
/// The upstream cannot be reached or a socket failed.
pub async fn forward(
    client: TcpStream,
    request: RequestHead,
    rest: Vec<u8>,
    upstream: &Upstream,
    map: &OriginMap,
    ended: impl Future<Output = ()>,
) -> std::io::Result<()> {
    tokio::select! {
        result = relay(client, request, rest, upstream, map) => result,
        () = ended => {
            tracing::debug!("a forwarded portal connection ended with its device or the portal");
            Ok(())
        }
    }
}

/// The device's side copied to the upstream; aborted when dropped, so a forwarding that is
/// given up leaves no task holding either connection.
struct Upload(JoinHandle<std::io::Result<u64>>);

impl Drop for Upload {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn relay(
    mut client: TcpStream,
    request: RequestHead,
    rest: Vec<u8>,
    upstream: &Upstream,
    map: &OriginMap,
) -> std::io::Result<()> {
    let upgrade = is_upgrade(&request);
    let host = upstream
        .host()
        .trim_start_matches('[')
        .trim_end_matches(']');
    let connect = TcpStream::connect((host, upstream.port()));
    let mut server = match tokio::time::timeout(UPSTREAM_TIMEOUT, connect).await {
        Ok(Ok(server)) => server,
        Ok(Err(error)) => {
            let _ = bad_gateway(&mut client).await;
            return Err(error);
        }
        Err(_) => {
            let _ = bad_gateway(&mut client).await;
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "the upstream did not answer",
            ));
        }
    };
    server
        .write_all(request_head(&request, upstream, map, upgrade).as_bytes())
        .await?;
    server.write_all(&rest).await?;
    let (mut client_read, mut client_write) = client.into_split();
    let (mut server_read, mut server_write) = server.into_split();
    // The request body (and, after an upgrade, the device's frames) goes on as it comes.
    let upload = Upload(tokio::spawn(async move {
        tokio::io::copy(&mut client_read, &mut server_write).await
    }));
    let read = tokio::time::timeout(UPSTREAM_TIMEOUT, read_response_head(&mut server_read)).await;
    let (head, mut body) = match read {
        Ok(Ok(read)) => read,
        Ok(Err(_)) | Err(_) => {
            drop(upload);
            let _ = client_write.write_all(BAD_GATEWAY.as_bytes()).await;
            return Ok(());
        }
    };
    let rewrite = !upgrade
        && request.method != "HEAD"
        && head.status == 200
        && (head.is_html() || head.is_css_or_script() || head.is_json())
        && !head.is_encoded();
    // The agent goes into the page's own documents only; an asset listener serves no agent.
    if rewrite && head.is_html() && upstream.origin() == map.main_upstream() {
        match read_body_within(&mut server_read, &mut body, &head, UPSTREAM_TIMEOUT).await {
            Ok(WholeBody::Whole(document)) => {
                let page = inject(&map.rewrite_body(&document));
                log_unmapped_loopback(&document, map);
                let out = response_head(&head, map, OutBody::Length(page.len()));
                client_write.write_all(out.as_bytes()).await?;
                client_write.write_all(&page).await?;
                drop(upload);
                return client_write.shutdown().await;
            }
            // Too large: it streams on rewritten, without the agent (`body` holds what came).
            Ok(WholeBody::TooLarge) => {
                tracing::debug!("a portal page was too large to take the agent");
            }
            Ok(WholeBody::Cut) => {
                tracing::debug!(
                    "the upstream's page ended before its body did or its chunks are malformed"
                );
                drop(upload);
                let _ = client_write.write_all(BAD_GATEWAY.as_bytes()).await;
                return client_write.shutdown().await;
            }
            Err(error) => {
                drop(upload);
                let _ = client_write.write_all(BAD_GATEWAY.as_bytes()).await;
                return Err(error);
            }
        }
    }
    if rewrite {
        let streamed =
            stream_rewritten(&mut server_read, &mut client_write, &head, map, &body).await;
        drop(upload);
        streamed?;
        return client_write.shutdown().await;
    }
    let out = response_head(&head, map, OutBody::Upstream);
    client_write.write_all(out.as_bytes()).await?;
    client_write.write_all(&body).await?;
    let copied = tokio::io::copy(&mut server_read, &mut client_write).await;
    // The upstream is done (or gone): nothing the device still sends has anywhere to go.
    drop(upload);
    copied?;
    client_write.shutdown().await
}

const BAD_GATEWAY: &str = "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain\r\n\
                           Content-Length: 11\r\nConnection: close\r\n\r\nBad Gateway";

async fn bad_gateway(client: &mut TcpStream) -> std::io::Result<()> {
    client.write_all(BAD_GATEWAY.as_bytes()).await?;
    client.shutdown().await
}

/// Whether the device asks for a WebSocket (or another protocol) upgrade.
fn is_upgrade(request: &RequestHead) -> bool {
    request.header("upgrade").is_some()
        && request.header("connection").is_some_and(|value| {
            value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
        })
}

/// The upstream's response head and the body bytes read with it.
async fn read_response_head(
    server: &mut (impl AsyncRead + Unpin),
) -> Result<(ResponseHead, Vec<u8>), HeadError> {
    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    loop {
        let read = server
            .read(&mut chunk)
            .await
            .map_err(|_| HeadError::Malformed)?;
        if read == 0 {
            return Err(HeadError::Malformed);
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(end) = head_end(&buffer) {
            let head = parse_response_head(&buffer[..end])?;
            return Ok((head, buffer.split_off(end)));
        }
        if buffer.len() > MAX_HEAD {
            return Err(HeadError::TooLarge);
        }
    }
}

/// What reading a body whole gave.
#[derive(Debug, PartialEq, Eq)]
enum WholeBody {
    /// The body, decoded.
    Whole(Vec<u8>),
    /// It grew past [`MAX_HTML`]; what was read stays in the caller's buffer.
    TooLarge,
    /// The upstream closed before the body ended, or its chunks are malformed.
    Cut,
}

/// [`read_body`] within `limit` in all: an upstream that sends an HTML page slowly or never
/// ends it cannot hold the connection (Charge 10 review L2).
async fn read_body_within(
    server: &mut (impl AsyncRead + Unpin),
    body: &mut Vec<u8>,
    head: &ResponseHead,
    limit: Duration,
) -> std::io::Result<WholeBody> {
    tokio::time::timeout(limit, read_body(server, body, head))
        .await
        .unwrap_or_else(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "the upstream's page did not end in time",
            ))
        })
}

/// Reads the whole body into `body` (which holds what came with the head) and answers it
/// decoded, or why not ([`WholeBody`]; `body` then holds what was read).
async fn read_body(
    server: &mut (impl AsyncRead + Unpin),
    body: &mut Vec<u8>,
    head: &ResponseHead,
) -> std::io::Result<WholeBody> {
    let length = head.content_length();
    let chunked = head.is_chunked();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        if let Some(end) = complete(body, length, chunked) {
            let whole = &body[..end];
            let decoded = if chunked {
                decode_chunked(whole)
            } else {
                Some(whole.to_vec())
            };
            return Ok(decoded.map_or(WholeBody::Cut, WholeBody::Whole));
        }
        if body.len() > MAX_HTML {
            return Ok(WholeBody::TooLarge);
        }
        let read = server.read(&mut chunk).await?;
        if read == 0 {
            // The upstream closed: the body is what came, unless it was cut short.
            let cut = chunked || length.is_some_and(|length| body.len() < length);
            return Ok(if cut {
                WholeBody::Cut
            } else {
                WholeBody::Whole(body.clone())
            });
        }
        body.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(test)]
#[path = "proxy_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "proxy_stream_tests.rs"]
mod stream_tests;
