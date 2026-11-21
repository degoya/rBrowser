//! What makes a portal listener the presentation's (RB-096-05): the tunnel's public host, known
//! once cloudflared named it - until then every request answers 503, after it every request
//! must name it in `Host` - the paths a viewer may load, and the pointer flash.

use std::sync::Mutex;

use super::paths::PathRules;
use super::tunnel::QUICK_TUNNEL_DOMAIN;
use crate::cdp::lock;

/// The presentation's side of its listener.
#[derive(Debug)]
pub struct Public {
    host: Mutex<Option<String>>,
    rules: PathRules,
    pointer: bool,
}

impl Public {
    /// No host yet; `rules` for the paths, `pointer` for the click flash.
    #[must_use]
    pub fn new(rules: PathRules, pointer: bool) -> Self {
        Self {
            host: Mutex::new(None),
            rules,
            pointer,
        }
    }

    /// The tunnel's address `https://<name>.trycloudflare.com` is known; `false` (and nothing
    /// set) for any other.
    pub fn set_address(&self, url: &str) -> bool {
        let Some(host) = url
            .strip_prefix("https://")
            .filter(|host| host.ends_with(QUICK_TUNNEL_DOMAIN) && !host.contains(['/', ':']))
        else {
            return false;
        };
        *lock(&self.host) = Some(host.to_owned());
        true
    }

    /// `<name>.trycloudflare.com`, once known.
    #[must_use]
    pub fn host(&self) -> Option<String> {
        lock(&self.host).clone()
    }

    /// `https://<name>.trycloudflare.com`, once known.
    #[must_use]
    pub fn origin(&self) -> Option<String> {
        self.host().map(|host| format!("https://{host}"))
    }

    /// The paths a viewer may load.
    #[must_use]
    pub fn rules(&self) -> &PathRules {
        &self.rules
    }

    /// Whether the viewer's page flashes where the presenter clicks.
    #[must_use]
    pub fn pointer(&self) -> bool {
        self.pointer
    }
}
