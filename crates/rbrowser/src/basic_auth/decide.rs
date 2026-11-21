//! Whether a Basic auth challenge is answered at all: only a server's (never a proxy's) `Basic`
//! challenge whose origin - scheme, host and port - is the origin of an environment that the
//! project lists under `basicAuth.environments`. A second site the page loads from, a redirect
//! target on another host, the same host on another port or scheme, plain `http` to another
//! machine and an address carrying user info are all refused, so the credential reaches nothing
//! but the environment it was set for.

use rb_config::ProjectConfig;

use super::origin::{AuthOrigin, OriginError};

/// A challenge as CEF's `GetAuthCredentials` reports it.
#[derive(Clone, Copy, Debug)]
pub struct Challenge<'a> {
    /// The origin making the request (the server that sent the challenge).
    pub origin_url: &'a str,
    /// Whether a proxy asks.
    pub is_proxy: bool,
    /// The host of the server.
    pub host: &'a str,
    /// Its port.
    pub port: i32,
    /// The realm the server names; a second realm of one origin is answered once on its own.
    pub realm: &'a str,
    /// `basic`, `digest`, ...; empty for FTP.
    pub scheme: &'a str,
}

/// Why a challenge is left to the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// A proxy asks.
    #[error("a proxy asks")]
    Proxy,
    /// Not the `Basic` scheme.
    #[error("not a Basic challenge")]
    Scheme,
    /// The origin may not carry a credential.
    #[error("{0}")]
    Origin(OriginError),
    /// The host or port CEF names is not the origin's.
    #[error("the challenge's host and origin differ")]
    HostPort,
    /// No environment behind Basic auth has this origin.
    #[error("no environment behind Basic auth has this origin")]
    NotAnEnvironment,
    /// No credential is stored for the environment.
    #[error("no credential is stored")]
    NoCredential,
    /// The view was answered once already: the server refused the credential.
    #[error("the stored credential was refused once")]
    Refused,
    /// No project is open.
    #[error("no project is open")]
    NoProject,
}

/// The environment a challenge is answered for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eligible {
    /// The environment's id.
    pub environment: String,
    /// Its origin, which names the credential.
    pub origin: AuthOrigin,
}

/// The environment of `config` whose credential answers `challenge`, or why none does.
///
/// # Errors
///
/// The [`Refusal`]: the challenge is left to the person.
pub fn eligible(challenge: &Challenge<'_>, config: &ProjectConfig) -> Result<Eligible, Refusal> {
    if challenge.is_proxy {
        return Err(Refusal::Proxy);
    }
    if !challenge.scheme.eq_ignore_ascii_case("basic") {
        return Err(Refusal::Scheme);
    }
    let origin = AuthOrigin::parse(challenge.origin_url).map_err(Refusal::Origin)?;
    let port = u16::try_from(challenge.port).map_err(|_| Refusal::HostPort)?;
    if !origin.matches_host_port(challenge.host, port) {
        return Err(Refusal::HostPort);
    }
    config
        .environments
        .iter()
        .filter(|environment| config.basic_auth.uses(&environment.id))
        .find(|environment| AuthOrigin::parse(&environment.url).as_ref() == Ok(&origin))
        .map(|environment| Eligible {
            environment: environment.id.clone(),
            origin: origin.clone(),
        })
        .ok_or(Refusal::NotAnEnvironment)
}

/// The origin of the environment `id` of `config`, the one its credential is set for: `None`
/// for no such environment, the [`OriginError`] of a URL that may not carry a credential.
pub fn environment_origin(
    config: &ProjectConfig,
    id: &str,
) -> Option<Result<AuthOrigin, OriginError>> {
    config
        .environments
        .iter()
        .find(|environment| environment.id == id)
        .map(|environment| AuthOrigin::parse(&environment.url))
}

#[cfg(test)]
#[path = "decide_tests.rs"]
mod tests;
