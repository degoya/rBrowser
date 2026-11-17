//! The asset listeners (RB-090-65): a page served through the portal may load its styles and
//! scripts from another origin of the developer's machine - a Laravel app on
//! `http://localhost:8000` with its Vite dev server on `http://localhost:5173`. Each origin an
//! environment lists in `assetOrigins` gets a listener of its own on the portal's LAN address
//! (a port the OS picks), which forwards only to that origin, and only while the shown
//! environment lists it. A port per origin, not a path prefix: a module's root-relative imports
//! (`/@vite/client`, `/node_modules/.vite/...`) then resolve against the asset listener and reach
//! the dev server unchanged.
//!
//! **Who may ask.** The same first checks as the main listener (proxy requests, its own
//! `ip:port` as `Host`, the connection limits). A request with the portal's cookie is the
//! device's whose session it is; a wrong cookie is refused. A request without it - a module
//! script, its imports, a font or a `fetch` on another port are cross-origin and go without
//! credentials - is accepted only as `GET` or `HEAD` from the address a paired device paired
//! from, once that device came back with its cookie; the grant ends with the device's session
//! (disconnect, the portal's stop) and with the listener. What a listener forwarded - an HMR
//! socket, an event stream - ends as well when another environment is shown that does not list
//! its origin. The portal's own paths
//! (`/__rbrowser/`) are not served here: the agent and its socket stay on the main listener.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::SystemTime;

use rb_cdp::request::RequestHead;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

use super::origins::{AssetOrigins, Upstream, listened, upstream_for};
use super::pairing::session_of;
use super::server::{OWN_PREFIX, PortalServer, accept_loop, page, respond_text};
use crate::cdp::lock;

/// What a listener serves.
#[derive(Clone)]
pub(super) enum Role {
    /// The pairing, the agent, its socket and the page.
    Main,
    /// One asset origin.
    Asset(Arc<AssetRole>),
}

/// An asset listener's one target.
pub(super) struct AssetRole {
    /// The asset origin it forwards to.
    pub(super) upstream: Upstream,
    /// Its own `ip:port`, the only `Host` it accepts.
    pub(super) authority: String,
    /// Turns (or closes) when the listener is stopped or dropped: what it forwarded ends too.
    pub(super) stopped: watch::Receiver<bool>,
}

/// A running asset listener.
struct Listener {
    /// The asset origin.
    upstream: String,
    /// `http://ip:port`.
    origin: String,
    stop: watch::Sender<bool>,
}

/// The running asset listeners, one per asset origin.
#[derive(Default)]
pub(super) struct AssetListeners(Vec<Listener>);

impl PortalServer {
    /// The environments' asset origins are `assets` (from [`super::origins::asset_origins`]):
    /// a listener for each new origin, none any more for an origin no environment lists; the
    /// listener of an origin still listed keeps its port.
    pub fn set_assets(&self, assets: AssetOrigins) {
        let wanted = listened(&assets);
        {
            let mut shared = lock(&self.shared);
            if shared.assets == assets {
                return;
            }
            shared.assets = assets;
            shared.refresh_shown();
        }
        // The listeners' lock before the shared one, everywhere.
        let mut listeners = lock(&self.assets);
        if self.is_stopped() {
            return;
        }
        listeners.0.retain(|listener| {
            let keep = wanted.contains(&listener.upstream);
            if !keep {
                listener.stop.send_replace(true);
                tracing::info!(origin = %listener.upstream, "the portal stopped forwarding an asset origin");
            }
            keep
        });
        for origin in &wanted {
            if listeners
                .0
                .iter()
                .any(|listener| listener.upstream == *origin)
            {
                continue;
            }
            let Some(upstream) = Upstream::parse(origin) else {
                continue;
            };
            match self.listen_for(upstream) {
                Ok(listener) => listeners.0.push(listener),
                Err(error) => {
                    tracing::warn!(%error, %origin, "the portal cannot listen for an asset origin");
                }
            }
        }
        lock(&self.shared).asset_ports = ports_of(&listeners);
    }

    /// Each asset origin with a listener and that listener's origin (`http://ip:port`).
    #[must_use]
    pub fn asset_listeners(&self) -> Vec<(String, String)> {
        lock(&self.shared).asset_ports.clone()
    }

    /// Stops every asset listener and what it forwards (the portal stops).
    pub(super) fn stop_assets(&self) {
        let mut listeners = lock(&self.assets);
        for listener in listeners.0.drain(..) {
            listener.stop.send_replace(true);
        }
        lock(&self.shared).asset_ports.clear();
    }

    /// A listener for `upstream` on the portal's address, on a port the OS picks.
    fn listen_for(&self, upstream: Upstream) -> std::io::Result<Listener> {
        let listener = std::net::TcpListener::bind(SocketAddr::new(self.address().ip(), 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let listener = {
            let _entered = self.runtime.enter();
            TcpListener::from_std(listener)?
        };
        let (stop, stopped) = watch::channel(false);
        tracing::info!(origin = upstream.origin(), %address, "the portal forwards an asset origin");
        let origin = upstream.origin().to_owned();
        let role = Role::Asset(Arc::new(AssetRole {
            upstream,
            authority: address.to_string(),
            stopped: stopped.clone(),
        }));
        self.runtime
            .spawn(accept_loop(listener, self.me.clone(), stopped, role));
        Ok(Listener {
            upstream: origin,
            origin: format!("http://{address}"),
            stop,
        })
    }
}

fn ports_of(listeners: &AssetListeners) -> Vec<(String, String)> {
    listeners
        .0
        .iter()
        .map(|listener| (listener.upstream.clone(), listener.origin.clone()))
        .collect()
}

/// A request on the asset listener of `asset`, its `Host` checked already.
pub(super) async fn serve(
    mut stream: TcpStream,
    request: RequestHead,
    rest: Vec<u8>,
    peer: IpAddr,
    server: &Arc<PortalServer>,
    asset: &AssetRole,
) {
    // The device's revocation is watched from the same lock that proved it.
    let device = {
        let now = SystemTime::now();
        let mut shared = lock(&server.shared);
        let found = match session_of(request.header("cookie")) {
            Some(offered) => shared.pairings.authenticate(offered, now),
            None if matches!(request.method.as_str(), "GET" | "HEAD") => {
                shared.pairings.device_at(peer, now)
            }
            None => None,
        };
        found.map(|device| shared.watch_device(device))
    };
    let Some(revoked) = device else {
        let _ = page(&mut stream, 403, "portal.page.not_paired", &[]).await;
        return;
    };
    if request.path().starts_with(OWN_PREFIX) {
        let _ = respond_text(&mut stream, 404, "Not Found").await;
        return;
    }
    // Only while the shown environment lists this origin; its list is watched from the same
    // lock that checked it, so a switch in between cannot be missed.
    let route = {
        let shared = lock(&server.shared);
        let listed = shared
            .shown
            .borrow()
            .iter()
            .any(|origin| origin == asset.upstream.origin());
        upstream_for(shared.current_url.as_deref(), &shared.allowlist)
            .ok()
            .filter(|_| listed)
            .map(|shown| {
                (
                    shared.origin_map(&shown, server.origin()),
                    shared.shown.subscribe(),
                )
            })
    };
    let Some((map, mut shown)) = route else {
        let _ = page(&mut stream, 403, "portal.page.not_allowed", &[]).await;
        return;
    };
    let device_ended = server.ended(revoked);
    let mut listener = asset.stopped.clone();
    let origin = asset.upstream.origin().to_owned();
    // The device is disconnected, the portal or this listener stops, or another environment
    // is shown that does not list the origin: what was forwarded ends.
    let ended = async move {
        tokio::select! {
            () = device_ended => {}
            result = listener.wait_for(|stopped| *stopped) => drop(result),
            result = shown.wait_for(|list| !list.contains(&origin)) => drop(result),
        }
    };
    if let Err(error) =
        super::proxy::forward(stream, request, rest, &asset.upstream, &map, ended).await
    {
        // The error only: the target may carry the page's own secrets.
        tracing::debug!(%error, "a portal asset request to its origin failed");
    }
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod tests;
