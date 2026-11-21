//! The presentation's methods on its [`PortalServer`] (RB-096-05): starting the listener on
//! loopback, issuing a viewer link, the link's time left and the viewers.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::runtime::Handle;

use super::link::link_url;
use super::public::Public;
use crate::cdp::lock;
use crate::portal::server::{Options, PortalServer};

/// A fresh viewer link: the URL to hand out (it holds the token: the panel only) and its life.
pub struct ViewerLink {
    /// `https://<name>.trycloudflare.com/__rbrowser/join#t=<token>`.
    pub url: String,
    /// Its life from now.
    pub expires_in: Duration,
}

impl std::fmt::Debug for ViewerLink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ViewerLink")
            .field("expires_in", &self.expires_in)
            .finish_non_exhaustive()
    }
}

impl PortalServer {
    /// The presentation's listener on 127.0.0.1 (`options.ip` is ignored), for cloudflared
    /// only: nothing in the LAN reaches it.
    ///
    /// # Errors
    ///
    /// No port is free on loopback.
    pub fn start_public(
        runtime: &Handle,
        options: Options,
        public: Arc<Public>,
    ) -> std::io::Result<Arc<Self>> {
        let options = Options {
            ip: Ipv4Addr::LOCALHOST,
            ..options
        };
        Self::start_with(runtime, options, Some(public))
    }

    /// Whether this is the presentation's listener.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.public.is_some()
    }

    /// The presentation's side; `None` on a LAN listener.
    #[must_use]
    pub fn public(&self) -> Option<&Arc<Public>> {
        self.public.as_ref()
    }

    /// A new viewer link (the one before is void); `None` before the tunnel named its address,
    /// and on a LAN listener. The token and the link join the log mask.
    ///
    /// # Errors
    ///
    /// The OS gives no randomness.
    pub fn issue_link(&self) -> std::io::Result<Option<ViewerLink>> {
        let Some(origin) = self.public.as_ref().and_then(|public| public.origin()) else {
            return Ok(None);
        };
        let now = Instant::now();
        let mut shared = lock(&self.shared);
        let token = shared.link.issue(now)?;
        let url = link_url(&origin, token.expose());
        rb_diag::register_secret(token.expose());
        rb_diag::register_secret(&url);
        let expires_in = shared
            .link
            .expires()
            .map_or(Duration::ZERO, |at| at.saturating_duration_since(now));
        drop(shared);
        self.changed();
        Ok(Some(ViewerLink { url, expires_in }))
    }

    /// The open link's time left at `now`; `None` when none is open.
    #[must_use]
    pub fn link_left(&self, now: Instant) -> Option<Duration> {
        let shared = lock(&self.shared);
        shared
            .link
            .is_open(now)
            .then(|| shared.link.expires())
            .flatten()
            .map(|at| at.saturating_duration_since(now))
    }
}
