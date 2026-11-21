//! The portal's listener (RB-080-05) on one LAN address: one task per connection reads the
//! request head and decides. Only four things are served - the pairing (`/__rbrowser/pair`),
//! the portal agent (`/__rbrowser/agent.js`), the device's WebSocket (`/__rbrowser/ws`) and the
//! project's page through [`super::proxy`]; nothing of MCP, CDP or the shell. Every request but
//! the pairing needs the device cookie; a `Host` other than the portal's own, a proxy request
//! (`GET http://...`, `CONNECT`) and a page outside the project's origins answer 403. Nothing
//! logged here carries a path, a query or a header: the pairing token is in the query and the
//! session in the cookie. The asset listeners ([`super::assets`], RB-090-65) take the same
//! accept loop and the same first checks, then serve only their one asset origin.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime};

use rb_cdp::request::{HeadError, MAX_HEAD, RequestHead, head_end, parse_head, websocket_key};
use rb_plugin_types::html::escape;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Handle;
use tokio::sync::watch;

use super::assets::{AssetListeners, Role};
use super::inject::AGENT_PATH;
use super::limits::Limits;
pub use super::limits::{MAX_CONNECTIONS, MAX_PER_ADDRESS};
use super::origins::{host_matches, upstream_for};
use super::pairing::{session_of, set_cookie};
use super::present::public::Public;
use super::state::Shared;
use crate::cdp::lock;

/// How long a device may take to send its request head.
const HEAD_TIMEOUT: Duration = Duration::from_secs(10);
/// The pairing path.
pub const PAIR_PATH: &str = "/__rbrowser/pair";
/// The device WebSocket's path.
pub const SOCKET_PATH: &str = "/__rbrowser/ws";
/// The portal's own paths.
pub(super) const OWN_PREFIX: &str = "/__rbrowser/";

/// What the app hears from the portal: the device list changed.
pub type OnChange = Arc<dyn Fn() + Send + Sync>;

/// The running portal; [`Self::stop`] ends the listener and every device's socket.
pub struct PortalServer {
    address: SocketAddr,
    origin: String,
    pub(super) shared: Mutex<Shared>,
    pub(super) agent: Arc<str>,
    pub(super) stopped: watch::Sender<bool>,
    connections: Mutex<Limits>,
    pub(super) on_change: OnChange,
    /// Where the asset listeners run.
    pub(super) runtime: Handle,
    /// The portal itself, for the accept loops of the asset listeners started later.
    pub(super) me: Weak<PortalServer>,
    /// One listener per asset origin (RB-090-65).
    pub(super) assets: Mutex<AssetListeners>,
    /// The presentation's listener behind the tunnel (RB-096-05, [`super::present::serve`]).
    pub(super) public: Option<Arc<Public>>,
}

impl std::fmt::Debug for PortalServer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PortalServer")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

/// How the portal starts.
pub struct Options {
    /// The address to listen on ([`super::interface::listen_address`]).
    pub ip: Ipv4Addr,
    /// A fixed port; a free one when `None`.
    pub port: Option<u16>,
    /// How long a pairing token lives.
    pub pairing_ttl: Duration,
    /// The most devices at once.
    pub max_devices: usize,
    /// The portal agent's source.
    pub agent: Arc<str>,
    /// Called when the device list changed, outside every lock.
    pub on_change: OnChange,
}

impl PortalServer {
    /// Listens on `options.ip` and serves on `runtime`.
    ///
    /// # Errors
    ///
    /// The address is no local one, or the port is taken.
    pub fn start(runtime: &Handle, options: Options) -> std::io::Result<Arc<Self>> {
        Self::start_with(runtime, options, None)
    }

    /// [`Self::start`], as the presentation's listener when `public` is given (RB-096-05).
    pub(super) fn start_with(
        runtime: &Handle,
        options: Options,
        public: Option<Arc<Public>>,
    ) -> std::io::Result<Arc<Self>> {
        let wanted = SocketAddr::from((options.ip, options.port.unwrap_or(0)));
        let listener = std::net::TcpListener::bind(wanted)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let listener = {
            let _entered = runtime.enter();
            TcpListener::from_std(listener)?
        };
        let (stopped, stop) = watch::channel(false);
        let server = Arc::new_cyclic(|me| Self {
            address,
            origin: format!("http://{address}"),
            shared: Mutex::new(Shared::new(options.pairing_ttl, options.max_devices)),
            agent: options.agent,
            stopped,
            // Behind the tunnel every connection comes from 127.0.0.1.
            connections: Mutex::new(if public.is_some() {
                Limits::shared_address()
            } else {
                Limits::default()
            }),
            on_change: options.on_change,
            runtime: runtime.clone(),
            me: me.clone(),
            assets: Mutex::new(AssetListeners::default()),
            public,
        });
        runtime.spawn(accept_loop(
            listener,
            Arc::downgrade(&server),
            stop,
            Role::Main,
        ));
        Ok(server)
    }

    /// `ip:port`.
    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// `http://ip:port`.
    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Stops listening (the asset listeners too), ends every device's socket and forwarded
    /// connection, revokes every session and takes the portal's secrets out of the log mask.
    pub fn stop(&self) {
        self.stopped.send_replace(true);
        self.stop_assets();
        self.forget_all();
    }

    /// Whether [`Self::stop`] was called.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        *self.stopped.borrow()
    }

    pub(super) fn changed(&self) {
        (self.on_change)();
    }
}

/// Accepts on `listener` until `stop` turns (or its sender is gone); each connection is
/// served in its `role`.
pub(super) async fn accept_loop(
    listener: TcpListener,
    server: Weak<PortalServer>,
    mut stop: watch::Receiver<bool>,
    role: Role,
) {
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        tracing::warn!(%error, "a portal connection was not accepted");
                        continue;
                    }
                };
                let Some(server) = server.upgrade() else {
                    return;
                };
                if !lock(&server.connections).acquire(peer.ip()) {
                    drop(stream);
                    continue;
                }
                let slot = Slot { server, ip: peer.ip() };
                let role = role.clone();
                tokio::spawn(async move {
                    serve(stream, slot.ip, &slot.server, &role).await;
                    drop(slot);
                });
            }
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    return;
                }
            }
        }
    }
}

/// A connection's place in [`Limits`], given back however its task ends.
struct Slot {
    server: Arc<PortalServer>,
    ip: IpAddr,
}

impl Drop for Slot {
    fn drop(&mut self) {
        lock(&self.server.connections).release(self.ip);
    }
}

async fn serve(mut stream: TcpStream, peer: IpAddr, server: &Arc<PortalServer>, role: &Role) {
    let (head, rest) = match tokio::time::timeout(HEAD_TIMEOUT, read_head(&mut stream)).await {
        Ok(Ok(read)) => read,
        Ok(Err(HeadError::TooLarge)) => {
            let _ = respond_text(&mut stream, 431, "Request Header Fields Too Large").await;
            return;
        }
        Ok(Err(HeadError::Malformed)) | Err(_) => return,
    };
    if is_proxy_request(&head) {
        let _ = page(&mut stream, 403, "portal.page.not_allowed", &[]).await;
        return;
    }
    let Ok(request) = parse_head(&head) else {
        let _ = respond_text(&mut stream, 400, "Bad Request").await;
        return;
    };
    if let Some(public) = &server.public {
        super::present::serve::serve(stream, request, rest, peer, server, public).await;
        return;
    }
    let authority = match role {
        Role::Main => server.address.to_string(),
        Role::Asset(asset) => asset.authority.clone(),
    };
    if !host_matches(request.header("host"), &authority) {
        tracing::debug!("a portal request named another host");
        let _ = page(&mut stream, 403, "portal.page.not_allowed", &[]).await;
        return;
    }
    if let Role::Asset(asset) = role {
        super::assets::serve(stream, request, rest, peer, server, asset).await;
        return;
    }
    let path = request.path().to_owned();
    if path == PAIR_PATH {
        pair(&mut stream, &request, peer, server).await;
        return;
    }
    // The device's revocation is watched from the same lock that proved it, so a disconnect
    // in between cannot be missed.
    let device = {
        let offered = session_of(request.header("cookie")).unwrap_or("");
        let mut shared = lock(&server.shared);
        shared
            .pairings
            .authenticate(offered, SystemTime::now())
            .map(|device| (device, shared.watch_device(device)))
    };
    let Some((device, revoked)) = device else {
        let _ = page(&mut stream, 403, "portal.page.not_paired", &[]).await;
        return;
    };
    if path == AGENT_PATH {
        let _ = respond(
            &mut stream,
            200,
            "OK",
            "application/javascript",
            server.agent.as_bytes(),
            &[],
        )
        .await;
        return;
    }
    if path == SOCKET_PATH {
        if let Some(key) = websocket_key(&request) {
            let key = key.to_owned();
            super::socket::run(stream, &key, device, server).await;
        } else {
            let _ = respond_text(&mut stream, 400, "Bad Request").await;
        }
        return;
    }
    if path.starts_with(OWN_PREFIX) {
        let _ = respond_text(&mut stream, 404, "Not Found").await;
        return;
    }
    let route = {
        let shared = lock(&server.shared);
        upstream_for(shared.current_url.as_deref(), &shared.allowlist)
            .map(|upstream| (shared.origin_map(&upstream, &server.origin), upstream))
    };
    match route {
        Ok((map, upstream)) => {
            let ended = server.ended(revoked);
            if let Err(error) =
                super::proxy::forward(stream, request, rest, &upstream, &map, ended).await
            {
                // The error only: the target may carry the page's own secrets.
                tracing::debug!(%error, "a portal request to the upstream failed");
            }
        }
        Err(refusal) => {
            let _ = page(&mut stream, 403, refusal.message_key(), &[]).await;
        }
    }
}

/// `/__rbrowser/pair?t=<token>`: the token for a device cookie, then on to `/`.
async fn pair(
    stream: &mut TcpStream,
    request: &RequestHead,
    peer: IpAddr,
    server: &Arc<PortalServer>,
) {
    let offered = token_of(&request.target).unwrap_or("");
    let user_agent = request.header("user-agent").unwrap_or("");
    let redeemed = lock(&server.shared).pairings.redeem(
        offered,
        peer,
        user_agent,
        Instant::now(),
        SystemTime::now(),
    );
    match redeemed {
        Ok((device, session)) => {
            rb_diag::register_secret(session.expose());
            tracing::info!(device, %peer, "a device paired with the portal");
            server.changed();
            let cookie = set_cookie(&session);
            // Not a `303`: a QR scanner app's navigation is cross-site, and the `Strict` cookie
            // set here would not follow a redirect of that chain. The page goes on to `/` itself,
            // a same-site navigation that carries the cookie.
            let body = paired_html();
            let _ = respond(
                stream,
                200,
                "OK",
                "text/html",
                body.as_bytes(),
                &[("Set-Cookie", &cookie)],
            )
            .await;
        }
        Err(error) => {
            tracing::info!(%peer, ?error, "a portal pairing was refused");
            let _ = page(stream, 403, error.message_key(), &[]).await;
        }
    }
}

/// The `t` parameter of the target's query.
fn token_of(target: &str) -> Option<&str> {
    let (_, query) = target.split_once('?')?;
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| *name == "t")
        .map(|(_, value)| value)
}

/// A request line that asks the portal to be a proxy: an absolute target or `CONNECT`.
fn is_proxy_request(head: &[u8]) -> bool {
    let line_end = head
        .windows(2)
        .position(|pair| pair == b"\r\n")
        .unwrap_or(head.len());
    let line = String::from_utf8_lossy(&head[..line_end]);
    let mut parts = line.split(' ');
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    method.eq_ignore_ascii_case("CONNECT") || (!target.is_empty() && !target.starts_with('/'))
}

/// The request head (up to its blank line) and the bytes read after it.
async fn read_head(stream: &mut TcpStream) -> Result<(Vec<u8>, Vec<u8>), HeadError> {
    let mut buffer = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    loop {
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|_| HeadError::Malformed)?;
        if read == 0 {
            return Err(HeadError::Malformed);
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(end) = head_end(&buffer) {
            let rest = buffer.split_off(end);
            return Ok((buffer, rest));
        }
        if buffer.len() > MAX_HEAD {
            return Err(HeadError::TooLarge);
        }
    }
}

/// A page with the text of `key` (the device's browser shows it).
pub(super) async fn page(
    stream: &mut TcpStream,
    status: u16,
    key: &str,
    args: &[(&str, &str)],
) -> std::io::Result<()> {
    let body = page_html(key, args);
    let phrase = if status == 403 { "Forbidden" } else { "Error" };
    respond(stream, status, phrase, "text/html", body.as_bytes(), &[]).await
}

/// The page after a pairing: it goes on to `/` by script (`location.replace`, no history
/// entry), by a refresh without script.
#[must_use]
pub fn paired_html() -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
         content=\"width=device-width\"><meta http-equiv=\"refresh\" content=\"0; url=/\">\
         <title>{title}</title></head><body><p>{text}</p>\
         <script>location.replace(\"/\")</script></body></html>",
        title = escape(&rb_i18n::t("portal.page.title")),
        text = escape(&rb_i18n::t("portal.page.paired")),
    )
}

/// The HTML of a portal page: the title and the text of `key`, escaped.
#[must_use]
pub fn page_html(key: &str, args: &[(&str, &str)]) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
         content=\"width=device-width\"><title>{title}</title></head><body><h1>{title}</h1>\
         <p>{text}</p></body></html>",
        title = escape(&rb_i18n::t("portal.page.title")),
        text = escape(&rb_i18n::t_with(key, args)),
    )
}

pub(super) async fn respond_text(
    stream: &mut TcpStream,
    status: u16,
    phrase: &str,
) -> std::io::Result<()> {
    respond(stream, status, phrase, "text/plain", phrase.as_bytes(), &[]).await
}

/// An answer of the portal itself: never cached, never a referrer, never framed.
pub(super) async fn respond(
    stream: &mut TcpStream,
    status: u16,
    phrase: &str,
    content_type: &str,
    body: &[u8],
    headers: &[(&str, &str)],
) -> std::io::Result<()> {
    let mut answer = format!(
        "HTTP/1.1 {status} {phrase}\r\nContent-Type: {content_type}; charset=UTF-8\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n\
         X-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        answer.push_str(name);
        answer.push_str(": ");
        answer.push_str(value);
        answer.push_str("\r\n");
    }
    answer.push_str("\r\n");
    stream.write_all(answer.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.shutdown().await
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "revoke_tests.rs"]
mod revoke_tests;

#[cfg(test)]
#[path = "device_miss_tests.rs"]
mod device_miss_tests;
