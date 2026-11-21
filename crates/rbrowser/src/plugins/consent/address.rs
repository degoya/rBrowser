//! Which pages a consent probe may load and run a plugin's collector in (RB-094-09, security
//! review): the rule of the plugins' `http` capability (`super::super::services::net`) - public
//! addresses only, or the trusted project's own sites (its `startUrl` and environments, none
//! while the workspace is untrusted) - unless the person switched on the protected setting
//! `plugins.consentProbeAnyUrl`. Every address a name resolves to must be public, since the
//! probe's browser picks one itself; the address of the page is checked before the load and
//! again before each collector runs, so a redirect or a script that moves the page into the LAN
//! stops the probe.
//!
//! The browser resolves the name again on its own: a DNS answer that changes between the check
//! and the load (rebinding) is not caught here - the residual risk of `docs/security/
//! threat-model.md`, T9.

use std::net::IpAddr;

use url::{Host, Url};

use crate::plugins::services::net::{Origin, is_public};

/// A page the probe may not load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused;

/// The rule a probe's addresses are held to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Admission {
    /// `plugins.consentProbeAnyUrl`: every address.
    pub any: bool,
    /// The trusted project's own origins, reached wherever they are.
    pub own: Vec<Origin>,
}

/// The host and port of an `http`/`https` URL as the browser reads it (WHATWG: `127.1` is
/// `127.0.0.1`), an IPv6 address without brackets.
#[must_use]
pub fn host_port(url: &str) -> Option<(String, u16)> {
    let url = Url::parse(url).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = match url.host()? {
        Host::Domain(name) => name.to_ascii_lowercase(),
        Host::Ipv4(ip) => ip.to_string(),
        Host::Ipv6(ip) => ip.to_string(),
    };
    Some((host, url.port_or_known_default()?))
}

/// The addresses `host` resolves to; an address literal is itself, a name that does not resolve
/// none.
async fn resolve(host: &str, port: u16) -> Vec<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return vec![ip];
    }
    match tokio::net::lookup_host((host, port)).await {
        Ok(addresses) => addresses.map(|address| address.ip()).collect(),
        Err(_) => Vec::new(),
    }
}

impl Admission {
    /// Whether the page `url`, its host resolved to `addresses`, may be probed.
    #[must_use]
    pub fn allows(&self, url: &str, addresses: &[IpAddr]) -> bool {
        if self.any {
            return true;
        }
        if host_port(url).is_none() {
            return false;
        }
        if Origin::of(url).is_some_and(|origin| self.own.contains(&origin)) {
            return true;
        }
        !addresses.is_empty() && addresses.iter().all(|address| is_public(*address))
    }

    /// [`Admission::allows`] with the addresses `url`'s host resolves to now.
    ///
    /// # Errors
    ///
    /// The page may not be probed.
    pub async fn check(&self, url: &str) -> Result<(), Refused> {
        if self.any {
            return Ok(());
        }
        let Some((host, port)) = host_port(url) else {
            return Err(Refused);
        };
        let addresses = resolve(&host, port).await;
        if self.allows(url, &addresses) {
            Ok(())
        } else {
            Err(Refused)
        }
    }
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod tests;
