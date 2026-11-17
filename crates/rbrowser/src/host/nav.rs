//! What the address bar and `navigate` accept, and the marker URL a new view starts with.

use rb_engine::{EngineError, ViewId};

/// The schemes a grid view may load. `javascript:` would run in the page under test and the
/// shell's own scheme belongs to the shell view only.
const ALLOWED_SCHEMES: [&str; 7] = [
    "http",
    "https",
    "file",
    "about",
    "data",
    "chrome",
    "view-source",
];

/// The prefix of the URL a new view loads until its CDP session is attached; the view id
/// follows. Its target is found by this URL, so no page URL is ever guessed (ADR 0001,
/// Decision 4).
pub const MARKER_PREFIX: &str = "about:blank#rb-view-";

/// The marker URL of `view`.
pub fn marker_url(view: ViewId) -> String {
    format!("{MARKER_PREFIX}{view}")
}

/// The view a marker URL names.
pub fn marker_view(url: &str) -> Option<ViewId> {
    url.strip_prefix(MARKER_PREFIX)?.parse().ok().map(ViewId)
}

/// Completes and checks a URL as typed: a missing scheme becomes `http://` for local hosts and
/// `https://` otherwise; schemes outside the allowed list are refused.
pub fn normalize_url(input: &str) -> Result<String, EngineError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(EngineError::Invalid("the URL is empty".to_owned()));
    }
    if text.chars().any(char::is_whitespace) {
        return Err(EngineError::Invalid(format!("not a URL: {text}")));
    }
    if let Some(scheme) = scheme_of(text) {
        let lower = scheme.to_ascii_lowercase();
        return if ALLOWED_SCHEMES.contains(&lower.as_str()) {
            Ok(text.to_owned())
        } else {
            Err(EngineError::Invalid(format!(
                "the scheme {lower}: cannot be opened in a view"
            )))
        };
    }
    let authority = text.split(['/', '?', '#']).next().unwrap_or_default();
    let host = match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => host,
        _ => authority,
    }
    .to_ascii_lowercase();
    let local = host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".test")
        || host == "[::1]"
        || host.parse::<std::net::Ipv4Addr>().is_ok();
    Ok(format!("{}://{text}", if local { "http" } else { "https" }))
}

/// The scheme of `text`, when it has one: letters, digits, `+ - .` before a `:` that is not
/// the port of a host (`localhost:3000`).
fn scheme_of(text: &str) -> Option<&str> {
    let (scheme, rest) = text.split_once(':')?;
    let well_formed = scheme
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    let is_port = rest
        .split(['/', '?', '#'])
        .next()
        .is_some_and(|port| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()));
    (well_formed && (!is_port || rest.starts_with("//"))).then_some(scheme)
}

#[cfg(test)]
#[path = "nav_tests.rs"]
mod tests;
