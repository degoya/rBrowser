//! The HTTP transport of the plugins' `http` capability (Charge 6): the host has checked the URL
//! (`http`/`https`, no credentials) and the rate already. Only `GET` and `HEAD` go out, with the
//! plugin's `Accept` and `Accept-Language` and no other header (no cookie, no authorization), no
//! proxy, a redirect is answered and never followed, at most `max_response_bytes` of the body
//! are read, and the answer carries only the headers [`RESPONSE_HEADERS`] names (a `Set-Cookie`
//! without its value).
//!
//! The names are resolved by [`GuardedResolver`]: every address of a name outside the project's
//! own origins must be public ([`super::net`]), and the connection goes only to an address it
//! let through, so DNS rebinding finds no second lookup to change. A refused address is
//! `plugins.error.http_url_denied`.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};

use rb_plugin_types::{Failure, FailureKind, HttpRequest, HttpResponse, codes};
use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{ArrayVec, DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

use super::net::{Origin, is_public};
use crate::sitemap::http::{TIMEOUT, outgoing};

/// The answer headers a plugin sees, lower-case: what a crawler and the SEO checks read, and the
/// security headers the security-headers check reads (RB-093-03). `set-cookie` passes without its
/// value ([`passed_value`]); a `cookie` never.
pub const RESPONSE_HEADERS: [&str; 18] = [
    "content-encoding",
    "content-language",
    "content-length",
    "content-type",
    "last-modified",
    "link",
    "location",
    "x-robots-tag",
    "content-security-policy",
    "strict-transport-security",
    "x-frame-options",
    "x-content-type-options",
    "referrer-policy",
    "permissions-policy",
    "access-control-allow-origin",
    "access-control-allow-credentials",
    "date",
    "set-cookie",
];

/// A header's value as a plugin gets it: a `Set-Cookie` line keeps its name and the attributes it
/// knows, each value checked against its form, and loses its value (`sid=SECRET; Secure` ->
/// `sid=; Secure`; a line without `=` passes as empty). What does not fit - a folded second
/// cookie, an unknown token, a quoted value split at its `;` - is left out, so no fragment of a
/// value reaches the plugin. Every other header passes as it came.
#[must_use]
pub fn passed_value(name: &str, value: &str) -> String {
    if name != "set-cookie" {
        return value.to_owned();
    }
    let mut parts = value.split(';');
    let Some((cookie, _)) = parts.next().and_then(|pair| pair.split_once('=')) else {
        return String::new();
    };
    let mut line = format!("{}=", cookie.trim());
    for attribute in parts.filter_map(known_attribute) {
        line.push_str("; ");
        line.push_str(&attribute);
    }
    line
}

/// `token` as a known `Set-Cookie` attribute whose value has its form, else `None`.
fn known_attribute(token: &str) -> Option<String> {
    let token = token.trim();
    let (key, value) = token.split_once('=').map_or((token, None), |(key, value)| {
        (key.trim(), Some(value.trim()))
    });
    let fits = |allowed: fn(char) -> bool, max: usize| {
        value.is_some_and(|value| {
            !value.is_empty() && value.len() <= max && value.chars().all(allowed)
        })
    };
    let ok = match key.to_ascii_lowercase().as_str() {
        "secure" | "httponly" | "partitioned" => value.is_none(),
        "samesite" | "priority" => fits(|c| c.is_ascii_alphabetic(), 16),
        "max-age" => fits(|c| c.is_ascii_digit() || c == '-', 20),
        "domain" => fits(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-', 253),
        "path" => fits(|c| c.is_ascii_graphic() && c != ',' && c != ';', 1024),
        "expires" => fits(|c| c.is_ascii_alphanumeric() || " ,:-".contains(c), 40),
        _ => false,
    };
    ok.then(|| token.to_owned())
}

/// The origins the plugins may reach on a non-public address: the open project's own.
pub type LocalOrigins = Arc<RwLock<Vec<Origin>>>;

/// Why a name was not resolved for a plugin: its addresses are not public.
#[derive(Debug)]
pub struct AddressRefused;

impl std::fmt::Display for AddressRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the address is not public")
    }
}

impl std::error::Error for AddressRefused {}

/// The addresses of `uri` a plugin may connect to: all of them for one of `local` origins, else
/// only the public ones - and none is an error.
///
/// # Errors
///
/// No address is left ([`AddressRefused`] inside `ureq::Error::Other`).
pub fn admitted(
    uri: &Uri,
    addresses: &[SocketAddr],
    local: &[Origin],
) -> Result<ResolvedSocketAddrs, ureq::Error> {
    let own = Origin::of_uri(uri).is_some_and(|origin| local.contains(&origin));
    let mut kept: ResolvedSocketAddrs =
        ArrayVec::from_fn(|_| SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)));
    for address in addresses
        .iter()
        .filter(|address| own || is_public(address.ip()))
    {
        if kept.try_push(*address).is_err() {
            break;
        }
    }
    if kept.is_empty() {
        Err(ureq::Error::Other(Box::new(AddressRefused)))
    } else {
        Ok(kept)
    }
}

/// The system resolver, its answer held to [`admitted`].
#[derive(Debug)]
pub struct GuardedResolver {
    inner: DefaultResolver,
    local: LocalOrigins,
}

impl Resolver for GuardedResolver {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let addresses = self.inner.resolve(uri, config, timeout)?;
        let local = self
            .local
            .read()
            .map(|local| local.clone())
            .unwrap_or_default();
        admitted(uri, &addresses, &local)
    }
}

/// The agent of the plugins' fetches.
pub fn agent(local: LocalOrigins) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .max_redirects(0)
        .http_status_as_error(false)
        .proxy(None)
        .user_agent(concat!("rBrowser/", env!("CARGO_PKG_VERSION"), " (plugin)"))
        .build();
    ureq::Agent::with_parts(
        config,
        DefaultConnector::default(),
        GuardedResolver {
            inner: DefaultResolver::default(),
            local,
        },
    )
}

fn failed(error: &ureq::Error) -> Failure {
    if let ureq::Error::Other(inner) = error
        && inner.is::<AddressRefused>()
    {
        return Failure::coded(FailureKind::AccessDenied, codes::HTTP_URL_DENIED)
            .with_param("reason", "address");
    }
    Failure::coded(FailureKind::Io, codes::HTTP_FAILED).with_param("reason", error.to_string())
}

/// One request on a blocking thread's time.
pub fn fetch_blocking(
    agent: &ureq::Agent,
    request: &HttpRequest,
    limit: u64,
) -> Result<HttpResponse, Failure> {
    let call = match request.method.to_ascii_uppercase().as_str() {
        "GET" => agent.get(&request.url),
        "HEAD" => agent.head(&request.url),
        _ => {
            return Err(
                Failure::coded(FailureKind::NotSupported, codes::HTTP_FAILED)
                    .with_param("reason", "only GET and HEAD"),
            );
        }
    };
    let call = outgoing(request)
        .into_iter()
        .fold(call, |call, (name, value)| call.header(name, value));
    let mut response = call.call().map_err(|error| failed(&error))?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter(|(name, _)| RESPONSE_HEADERS.contains(&name.as_str()))
        .filter_map(|(name, value)| {
            let value = passed_value(name.as_str(), value.to_str().ok()?);
            Some((name.as_str().to_owned(), value))
        })
        .collect();
    let body = if request.method.eq_ignore_ascii_case("HEAD") {
        Vec::new()
    } else {
        response
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_vec()
            .map_err(|error| {
                Failure::coded(FailureKind::Limit, codes::TOO_LARGE)
                    .with_param("reason", error.to_string())
            })?
    };
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
