//! The origin a Basic auth credential belongs to: scheme, host and port, as the `url` crate
//! normalises them (lower case, IDNA, `127.1` as `127.0.0.1`, the scheme's default port). Only
//! `https`, and plain `http` on the own machine (`localhost`, `*.localhost`, a loopback
//! address), may carry a credential; a URL with user info is refused, so a password written into
//! an address is never taken for one.

use std::fmt;

use url::{Host, Url};

/// Why a URL is no origin a credential may be sent to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OriginError {
    /// Not an absolute URL.
    #[error("not an absolute URL")]
    Invalid,
    /// Neither `https` nor `http`.
    #[error("only https (and http on this machine) carries Basic auth")]
    Scheme,
    /// Plain `http` to another machine: the password would cross the network in clear.
    #[error("plain http carries Basic auth only to this machine")]
    PlainHttp,
    /// A user name or password inside the URL.
    #[error("the address carries a user name or password")]
    UserInfo,
}

/// Scheme, host and port of an `http(s)` URL.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AuthOrigin {
    scheme: String,
    host: String,
    port: u16,
}

impl AuthOrigin {
    /// The origin of `url`.
    ///
    /// # Errors
    ///
    /// [`OriginError`] for a URL that may not carry a credential.
    pub fn parse(url: &str) -> Result<Self, OriginError> {
        let parsed = Url::parse(url.trim()).map_err(|_| OriginError::Invalid)?;
        let scheme = parsed.scheme();
        if scheme != "https" && scheme != "http" {
            return Err(OriginError::Scheme);
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(OriginError::UserInfo);
        }
        let host = parsed.host().ok_or(OriginError::Invalid)?;
        if scheme == "http" && !is_loopback(&host) {
            return Err(OriginError::PlainHttp);
        }
        let port = parsed.port_or_known_default().ok_or(OriginError::Invalid)?;
        Ok(Self {
            scheme: scheme.to_owned(),
            host: host.to_string(),
            port,
        })
    }

    /// `https` or `http`.
    #[must_use]
    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// The host, lower case; an IPv6 literal in brackets.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The port, the scheme's default when the URL names none.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Whether a challenge's `host` and `port`, as CEF reports them, are this origin's (an IPv6
    /// literal with or without its brackets).
    #[must_use]
    pub fn matches_host_port(&self, host: &str, port: u16) -> bool {
        let bare = |text: &str| {
            text.trim_start_matches('[')
                .trim_end_matches(']')
                .to_ascii_lowercase()
        };
        port == self.port && bare(host) == bare(&self.host)
    }
}

/// `scheme://host:port`, the port always written: the name of the credential.
impl fmt::Display for AuthOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}:{}", self.scheme, self.host, self.port)
    }
}

/// `localhost`, a name under `.localhost`, or a loopback address.
fn is_loopback(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(name) => *name == "localhost" || name.ends_with(".localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod tests;
