//! Where a load test may send (RB-096-07, a review condition): only to systems that are the
//! person's. Without asking: the hosts of the trusted project's own origins (`startUrl`, the
//! `environments`) and this machine (`localhost`, `*.localhost`, `127.0.0.0/8`, `[::1]`); any
//! other host only after the person confirmed in the panel, for that host, that it is theirs or
//! that they may test it. The confirmation is kept in `config.local.json` as
//! `load-test.allowedHosts` - this project, this machine, never checked in, `x-protected` so no
//! MCP client writes it - and taken back in Settings > Network > Load test. It is a list of its
//! own: a host the API client may send to is not yet one the load test may flood.
//!
//! A host is matched whole, never by suffix: neither a parent nor a sibling of an own host is
//! one. Every hop of every request - a redirect included - is held to [`Gate::allowed`] by the
//! API client's one send path (`crate::api_client::deliver`).

use std::net::Ipv4Addr;

use serde::Serialize;

use crate::api_client::grants::{host_of, normalized};

/// The settings key of the confirmed hosts.
pub const ALLOWED_HOSTS_KEY: &str = "load-test.allowedHosts";
/// This machine's hosts, always allowed.
pub const LOCAL_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

/// Why a host may be a target, or that it may not yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Standing {
    /// A host of the project's own origins.
    Own,
    /// This machine.
    Local,
    /// One the person confirmed is theirs or theirs to test.
    Confirmed,
    /// Any other: not before the person confirms it.
    Unconfirmed,
}

/// The hosts of one project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Gate {
    own: Vec<String>,
    confirmed: Vec<String>,
}

/// Whether `host` (normalized) is this machine.
#[must_use]
pub fn is_local(host: &str) -> bool {
    host == "localhost"
        || host.ends_with(".localhost")
        || host == "[::1]"
        || host
            .parse::<Ipv4Addr>()
            .is_ok_and(|address| address.is_loopback())
}

impl Gate {
    /// The gate of a project whose origins are `own_urls` (`startUrl`, the environments' URLs)
    /// and whose person confirmed `confirmed`.
    #[must_use]
    pub fn new(own_urls: &[String], confirmed: &[String]) -> Self {
        let mut own: Vec<String> = own_urls.iter().filter_map(|url| host_of(url)).collect();
        own.sort();
        own.dedup();
        let mut confirmed: Vec<String> = confirmed.iter().filter_map(|h| normalized(h)).collect();
        confirmed.sort();
        confirmed.dedup();
        Self { own, confirmed }
    }

    /// The hosts of the project's own origins.
    #[must_use]
    pub fn own(&self) -> &[String] {
        &self.own
    }

    /// The standing of `host`.
    #[must_use]
    pub fn standing(&self, host: &str) -> Standing {
        let Some(host) = normalized(host) else {
            return Standing::Unconfirmed;
        };
        if self.own.contains(&host) {
            Standing::Own
        } else if is_local(&host) {
            Standing::Local
        } else if self.confirmed.contains(&host) {
            Standing::Confirmed
        } else {
            Standing::Unconfirmed
        }
    }

    /// The `targets` not confirmed yet, each once.
    #[must_use]
    pub fn unconfirmed(&self, targets: &[String]) -> Vec<String> {
        let mut out: Vec<String> = targets
            .iter()
            .filter(|host| self.standing(host) == Standing::Unconfirmed)
            .cloned()
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The hosts every hop of a run with `targets` is held to: the own ones, this machine's,
    /// the confirmed ones and the targets that are this machine - never an unconfirmed one.
    #[must_use]
    pub fn allowed(&self, targets: &[String]) -> Vec<String> {
        let mut out: Vec<String> = self
            .own
            .iter()
            .chain(&self.confirmed)
            .cloned()
            .chain(LOCAL_HOSTS.iter().map(|host| (*host).to_owned()))
            .chain(
                targets
                    .iter()
                    .filter_map(|host| normalized(host))
                    .filter(|host| is_local(host)),
            )
            .collect();
        out.sort();
        out.dedup();
        out
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
