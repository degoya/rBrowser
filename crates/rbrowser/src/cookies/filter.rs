//! Which cookies of a session are copied (RB-040-05): those of the sites the views show and of
//! the current environment (`engines.cookies.domainFilter` `environment`, the default), every one
//! (`all`) or those of the sites MCP names (`domains`); never one on `engines.cookies.exclude`.
//!
//! A cookie belongs to a site when the site's host domain-matches the cookie's domain (RFC 6265
//! 5.1.3): `app.example.com` takes the cookies of `app.example.com` and `.example.com`, not
//! those of `api.example.com`. An exclusion entry is a cookie name, `*` matching any text
//! (`_ga*`), or a domain that covers its subdomains.

use super::model::{ChromeCookie, SkipReason};

/// The filter of one copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    /// The sites' hosts, lowercase; `None`: every site.
    hosts: Option<Vec<String>>,
    /// `engines.cookies.exclude`, trimmed, without empty entries.
    exclude: Vec<String>,
}

impl Filter {
    /// Cookies of `hosts` (every site with `None`) not on `exclude`.
    pub fn new(hosts: Option<Vec<String>>, exclude: &[String]) -> Self {
        let hosts = hosts.map(|hosts| {
            let mut hosts: Vec<String> = hosts.iter().filter_map(|host| normal(host)).collect();
            hosts.sort();
            hosts.dedup();
            hosts
        });
        let exclude = exclude
            .iter()
            .map(|entry| entry.trim().to_owned())
            .filter(|entry| !entry.is_empty())
            .collect();
        Self { hosts, exclude }
    }

    /// Why `cookie` is left out, if it is.
    pub fn check(&self, cookie: &ChromeCookie) -> Result<(), SkipReason> {
        let domain = cookie.domain.trim_start_matches('.').to_ascii_lowercase();
        let excluded = self.exclude.iter().any(|entry| {
            glob(entry, &cookie.name)
                || normal(entry).is_some_and(|entry| domain_matches(&domain, &entry))
        });
        if excluded {
            return Err(SkipReason::Excluded);
        }
        match &self.hosts {
            Some(hosts) if !hosts.iter().any(|host| domain_matches(host, &domain)) => {
                Err(SkipReason::OtherDomain)
            }
            _ => Ok(()),
        }
    }
}

/// Whether `host` domain-matches `domain` (both lowercase, no leading dot): the same, or a
/// subdomain of it.
pub fn domain_matches(host: &str, domain: &str) -> bool {
    !domain.is_empty()
        && (host == domain
            || host
                .strip_suffix(domain)
                .is_some_and(|rest| rest.ends_with('.')))
}

/// The host of an `http`/`https` URL as cookies compare it ([`normal`]): lowercase, without
/// scheme, port, user info or the brackets of an IPv6 literal.
pub fn cookie_host(url: &str) -> Option<String> {
    rb_util::url::host_of(url).and_then(|host| normal(&host))
}

/// A host or domain as compared: trimmed, lowercase, without a leading dot or brackets.
fn normal(host: &str) -> Option<String> {
    let host = host
        .trim()
        .trim_start_matches('.')
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// Whether `text` matches `pattern`, where `*` stands for any text.
fn glob(pattern: &str, text: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        // No `*`: the whole name.
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod tests;
