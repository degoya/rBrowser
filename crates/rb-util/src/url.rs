//! The origin, host and authority of a URL, without a URL crate: the views show web pages, so
//! `http(s)` (and `ws(s)` for a host) is all the app and its plugins compare.

/// The authority of `url` (`host[:port]`, an IPv6 literal in its brackets) as written, the user
/// info in front of the last `@` dropped; `None` without `://` or for an empty authority.
#[must_use]
pub fn authority_of(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    (!authority.is_empty()).then_some(authority)
}

/// The host of an `http(s)` or `ws(s)` URL, lower case, without user info and port; an IPv6
/// literal keeps its brackets (`[::1]`). `None` for another scheme or a URL without a host.
#[must_use]
pub fn host_of(url: &str) -> Option<String> {
    let scheme = scheme_of(url)?;
    if !matches!(scheme.as_str(), "http" | "https" | "ws" | "wss") {
        return None;
    }
    let (host, _) = split_port(authority_of(url)?);
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// The origin of an `http(s)` URL as `location.origin` gives it: `scheme://host[:port]` in lower
/// case, user info dropped, the scheme's default port (80, 443) dropped. `None` for another
/// scheme (`about:`, `file:`, `data:`, the app's own) or a URL without a host.
#[must_use]
pub fn origin_of(url: &str) -> Option<String> {
    let scheme = scheme_of(url)?;
    let default_port = match scheme.as_str() {
        "http" => "80",
        "https" => "443",
        _ => return None,
    };
    let (host, port) = split_port(authority_of(url)?);
    if host.is_empty() {
        return None;
    }
    let host = host.to_ascii_lowercase();
    match port {
        Some(port) if port != default_port => Some(format!("{scheme}://{host}:{port}")),
        _ => Some(format!("{scheme}://{host}")),
    }
}

/// The scheme in front of `://`, lower case.
fn scheme_of(url: &str) -> Option<String> {
    url.split_once("://")
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
}

/// `authority` as host and port; an IPv6 literal keeps its brackets.
fn split_port(authority: &str) -> (&str, Option<&str>) {
    if authority.starts_with('[') {
        let end = authority.find(']').map_or(authority.len(), |at| at + 1);
        let (host, rest) = authority.split_at(end);
        return (host, rest.strip_prefix(':'));
    }
    match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    }
}

#[cfg(test)]
#[path = "url_tests.rs"]
mod tests;
