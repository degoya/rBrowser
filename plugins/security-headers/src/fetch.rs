//! The document's answer, asked for once through the host's `http` capability: `HEAD` (a `GET`
//! when the server refuses `HEAD` with 405 or 501), redirects followed by hand up to
//! [`MAX_REDIRECTS`] (the host answers a redirect and never follows it), the `Set-Cookie` lines
//! of every hop kept for their attributes. The host sends no cookie and no credentials, reaches
//! public addresses and the project's own origins only, and bounds the answer.
//!
//! The headers are those of the final answer. The host hands a plugin only the answer headers
//! it lists (`RESPONSE_HEADERS` in `crates/rbrowser/src/plugins/services/http.rs`); an answer
//! that carries none of the headers this check reads and no `Date` - which every origin server
//! with a clock sends (RFC 9110, 6.6.1) - is taken as filtered, and the check fails with
//! `errors.headers_unavailable` instead of reporting every header missing.

use rb_plugin_api::{Failure, FailureKind, HttpRequest, PluginHost, rules};

use crate::cookies::{self, Cookie};
use crate::date;
use crate::headers::Headers;

/// The most redirects followed.
pub const MAX_REDIRECTS: usize = 5;
/// What the check asks for.
const ACCEPT: &str = "text/html,application/xhtml+xml,*/*;q=0.8";
/// The longest `Location` followed, in bytes.
const MAX_LOCATION_BYTES: usize = 2048;

/// The headers that tell a filtered answer from a real one.
pub const EVIDENCE: [&str; 9] = [
    "date",
    "content-security-policy",
    "strict-transport-security",
    "x-frame-options",
    "x-content-type-options",
    "referrer-policy",
    "permissions-policy",
    "access-control-allow-origin",
    "set-cookie",
];

/// The document's answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fetched {
    /// The URL asked for.
    pub url: String,
    /// The URL of the final answer.
    pub final_url: String,
    pub status: u16,
    pub headers: Headers,
    /// The cookies the answer and its redirects set, attributes only.
    pub cookies: Vec<Cookie>,
    pub redirects: usize,
    /// The first `http:` URL an `https:` page redirected to: the connection was downgraded.
    pub downgrade: Option<String>,
}

/// `plugin.security-headers.errors.<name>`.
pub(crate) fn error(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &crate::key(&format!("errors.{name}")))
}

/// Whether `url` is an `https:` URL.
pub(crate) fn is_https(url: &str) -> bool {
    url.get(..8)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

/// Whether `location` starts with a scheme (`https:`, `data:`), so it is no relative reference.
fn has_scheme(location: &str) -> bool {
    let end = location.find([':', '/', '?', '#']);
    end.is_some_and(|end| {
        location.get(end..end + 1) == Some(":")
            && location.get(..end).is_some_and(|scheme| {
                scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                    && scheme
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            })
    })
}

/// The absolute URL `location` names, seen from `base`; `None` for anything but `http(s)`.
#[must_use]
pub fn resolve(base: &str, location: &str) -> Option<String> {
    let location = location.trim();
    if location.is_empty() || location.len() > MAX_LOCATION_BYTES {
        return None;
    }
    let (scheme, rest) = base.split_once("://")?;
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let origin = format!("{scheme}://{}", rest.get(..authority_end)?);
    let resolved = if has_scheme(location) {
        location.to_owned()
    } else if let Some(network) = location.strip_prefix("//") {
        format!("{scheme}://{network}")
    } else if location.starts_with('/') {
        format!("{origin}{location}")
    } else if location.starts_with('?') || location.starts_with('#') {
        // A query or a fragment alone keeps the path (RFC 3986, 5.2.2); a query keeps no old one.
        let path = rest.get(authority_end..).unwrap_or_default();
        let cut = if location.starts_with('?') {
            path.split(['?', '#']).next()
        } else {
            path.split('#').next()
        };
        format!(
            "{origin}{}{location}",
            cut.filter(|path| !path.is_empty()).unwrap_or("/")
        )
    } else {
        let path = rest.get(authority_end..).unwrap_or_default();
        let path = path.split(['?', '#']).next().unwrap_or_default();
        let directory = path
            .rfind('/')
            .and_then(|at| path.get(..=at))
            .unwrap_or("/");
        format!("{origin}{directory}{location}")
    };
    rules::check_http_url(&resolved).ok().map(|()| resolved)
}

fn ask(
    host: &mut dyn PluginHost,
    method: &str,
    url: &str,
) -> Result<(u16, Vec<(String, String)>), Failure> {
    let answer = host.fetch(&HttpRequest {
        method: method.to_owned(),
        url: url.to_owned(),
        headers: vec![("accept".to_owned(), ACCEPT.to_owned())],
        body: Vec::new(),
    })?;
    Ok((answer.status, answer.headers))
}

/// The answer of the document at `url`.
///
/// # Errors
///
/// The host's refusal or failure of a request, a redirect loop longer than [`MAX_REDIRECTS`] or
/// to a URL that is not `http(s)` (`errors.redirect`), an answer the host filtered
/// (`errors.headers_unavailable`).
pub fn document(host: &mut dyn PluginHost, url: &str) -> Result<Fetched, Failure> {
    let mut current = url.to_owned();
    let mut set_cookies: Vec<Cookie> = Vec::new();
    let mut downgrade: Option<String> = None;
    for redirects in 0..=MAX_REDIRECTS {
        let (mut status, mut pairs) = ask(host, "HEAD", &current)?;
        if status == 405 || status == 501 {
            (status, pairs) = ask(host, "GET", &current)?;
        }
        let all = Headers::from_pairs(&pairs);
        // A deleting cookie's Expires is held to this answer's own clock.
        let now = all.get("date").and_then(|date| date::parse(&date));
        let hop: Vec<Cookie> = all
            .all("set-cookie")
            .filter_map(|line| cookies::parse_set_cookie_at(line, now))
            .collect();
        set_cookies = cookies::apply(set_cookies, hop);
        let evidence = EVIDENCE.iter().any(|name| all.has(name));
        // The cookies' values go no further than this.
        let headers = all.without("set-cookie");
        if (300..400).contains(&status)
            && let Some(location) = headers.get("location")
        {
            if redirects == MAX_REDIRECTS {
                break;
            }
            let next = resolve(&current, &location)
                .ok_or_else(|| error(FailureKind::BadData, "redirect"))?;
            if downgrade.is_none() && is_https(&current) && !is_https(&next) {
                downgrade = Some(next.clone());
            }
            current = next;
            continue;
        }
        if !evidence {
            return Err(error(FailureKind::NotSupported, "headers_unavailable"));
        }
        return Ok(Fetched {
            url: url.to_owned(),
            final_url: current,
            status,
            headers,
            cookies: set_cookies,
            redirects,
            downgrade,
        });
    }
    Err(error(FailureKind::BadData, "redirect").with_param("limit", MAX_REDIRECTS))
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
