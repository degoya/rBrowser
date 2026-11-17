//! The checks of `cdp` and `http` that need no disk and no network: which CDP domain a method
//! belongs to and whether the manifest lists it, and which URLs `http.fetch` reaches. Shared by
//! the host and the native test host, like [`crate::paths`].

use crate::{Failure, FailureKind, codes};

/// The longest URL `http.fetch` accepts, in bytes.
pub const MAX_URL_BYTES: usize = 8 * 1024;

/// The domain of a CDP method: `CSS` of `CSS.getComputedStyleForNode`.
#[must_use]
pub fn cdp_domain(method: &str) -> Option<&str> {
    let (domain, command) = method.split_once('.')?;
    let well_formed = !domain.is_empty()
        && !command.is_empty()
        && domain.chars().all(|c| c.is_ascii_alphanumeric())
        && command.chars().all(|c| c.is_ascii_alphanumeric());
    well_formed.then_some(domain)
}

/// `Ok` when `method` belongs to one of `domains` (the manifest's `cdp.domains`, compared
/// exactly); `plugins.error.cdp_domain_denied` naming the domain otherwise.
///
/// # Errors
///
/// The method is malformed or its domain is not listed.
pub fn check_cdp_method(method: &str, domains: &[String]) -> Result<(), Failure> {
    let domain = cdp_domain(method).unwrap_or(method);
    if cdp_domain(method).is_some() && domains.iter().any(|listed| listed == domain) {
        return Ok(());
    }
    let shown: String = domain.chars().take(64).collect();
    Err(
        Failure::coded(FailureKind::AccessDenied, codes::CDP_DOMAIN_DENIED)
            .with_param("domain", shown),
    )
}

/// `Ok` for an `http:` or `https:` URL with a host, no credentials and at most
/// [`MAX_URL_BYTES`]; `plugins.error.http_url_denied` otherwise. Development servers on
/// `localhost` are reachable on purpose: checking them is what the SEO and link checks are for.
///
/// # Errors
///
/// The URL is refused.
pub fn check_http_url(url: &str) -> Result<(), Failure> {
    let refused = || {
        let shown: String = url.chars().take(200).collect();
        Failure::coded(FailureKind::AccessDenied, codes::HTTP_URL_DENIED).with_param("url", shown)
    };
    if url.len() > MAX_URL_BYTES || url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(refused());
    }
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .ok_or_else(refused)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return Err(refused());
    }
    Ok(())
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
