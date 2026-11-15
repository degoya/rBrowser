//! Origins, hosts, paths and the cookie matching rules of RFC 6265 - the little URL handling the
//! plugin needs, without a URL crate: `http` and `https` only, as the views show web pages.

/// The origin (`scheme://host[:port]`, lower case, the default port dropped) and the host
/// (without its port) of a URL: `rb_util::url`'s, as the app reads them.
pub use rb_util::url::{host_of, origin_of};

/// Whether `text` is an origin exactly as [`origin_of`] writes it.
#[must_use]
pub fn is_origin(text: &str) -> bool {
    origin_of(text).as_deref() == Some(text)
}

/// The path of `url`, `/` when it has none.
#[must_use]
pub fn path_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    match rest.find('/') {
        Some(at) => rest[at..].to_owned(),
        None => "/".to_owned(),
    }
}

/// Whether a cookie of `domain` is sent to `host`: a domain cookie (leading dot) to the domain
/// and its subdomains, a host-only cookie to its host alone.
#[must_use]
pub fn domain_matches(host: &str, domain: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let domain = domain.to_ascii_lowercase();
    match domain.strip_prefix('.') {
        Some(base) => host == base || host.ends_with(&format!(".{base}")),
        None => host == domain,
    }
}

/// Whether a cookie of `cookie_path` is sent with a request for `path` (RFC 6265, 5.1.4).
#[must_use]
pub fn path_matches(path: &str, cookie_path: &str) -> bool {
    path == cookie_path
        || (path.starts_with(cookie_path)
            && (cookie_path.ends_with('/') || path[cookie_path.len()..].starts_with('/')))
}

/// Whether `domain` may be a cookie's domain: a host name or address of at most 253 characters,
/// letters, digits, `-`, `.`, `:` (IPv6) and brackets, with an optional leading dot.
#[must_use]
pub fn is_cookie_domain(domain: &str) -> bool {
    let bare = domain.strip_prefix('.').unwrap_or(domain);
    !bare.is_empty()
        && domain.len() <= 253
        && bare
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':' | '[' | ']'))
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod tests;
