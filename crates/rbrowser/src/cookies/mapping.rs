//! A Chromium cookie as Firefox (`storage.setCookie`) and WebKit (Playwright's `addCookies`)
//! take it (RB-040-05).
//!
//! | Chromium (CDP) | Firefox (BiDi) | WebKit (Playwright) |
//! | --- | --- | --- |
//! | `name`, `value`, `path` | the same | the same |
//! | `domain` `.example.com` / `example.com` | as is (a domain cookie either way) | as is (host-only without the dot) |
//! | `expires` -1 or `session` | no `expiry`: a session cookie | `expires: -1` |
//! | `expires` seconds | `expiry`, whole seconds | `expires`, whole seconds |
//! | `httpOnly`, `secure` | the same | the same; `secure` dropped on a loopback host |
//! | `sameSite` `Strict`/`Lax`/`None`, absent | `strict`/`lax`/`none`, `lax` | the same, `Lax` |
//! | `partitionKey` (CHIPS) | skipped, `partitioned` | skipped, `partitioned` |
//!
//! Absent `sameSite` is Chromium's "lax by default", so the copy behaves as it did in Chromium.
//! Chromium counts `localhost`, `127.0.0.1` and `[::1]` as secure and sends a `Secure` cookie
//! over plain HTTP there (the `sameSite=None; Secure` login of a local dev server); Firefox does
//! the same, WebKit does not, so its copy drops `Secure` on those hosts only - the cookie never
//! leaves the machine either way. An expired cookie, a partitioned one and one without a name,
//! domain or path are skipped with their reason.

use rb_bidi::storage::Cookie as BidiCookie;
use serde::Serialize;

use super::model::{ChromeCookie, SkipReason};

/// A cookie as Playwright's `addCookies` takes it (`sidecar/src/webkit/cookies.ts`). Its
/// `Debug` hides the value.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaywrightCookie {
    /// The name.
    pub name: String,
    /// The value: a secret.
    pub value: String,
    /// The domain; without a leading dot a host-only cookie.
    pub domain: String,
    /// The path.
    pub path: String,
    /// Seconds since the epoch; -1 for a session cookie.
    pub expires: i64,
    /// `HttpOnly`.
    pub http_only: bool,
    /// `Secure`.
    pub secure: bool,
    /// `Strict`, `Lax` or `None`.
    pub same_site: &'static str,
}

impl std::fmt::Debug for PlaywrightCookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlaywrightCookie")
            .field("name", &self.name)
            .field("value", &"<masked>")
            .field("domain", &self.domain)
            .finish_non_exhaustive()
    }
}

/// Why `cookie` cannot be copied at `now` (seconds since the epoch), if it cannot.
pub fn check(cookie: &ChromeCookie, now: u64) -> Result<(), SkipReason> {
    if cookie.partition_key.is_some() || cookie.partition_key_opaque {
        return Err(SkipReason::Partitioned);
    }
    if cookie.name.is_empty()
        || cookie.domain.trim_start_matches('.').is_empty()
        || !cookie.path.starts_with('/')
    {
        return Err(SkipReason::Invalid);
    }
    if expiry(cookie).is_some_and(|expiry| expiry <= now) {
        return Err(SkipReason::Expired);
    }
    Ok(())
}

/// `cookie` for Firefox.
pub fn to_bidi(cookie: &ChromeCookie, now: u64) -> Result<BidiCookie, SkipReason> {
    check(cookie, now)?;
    Ok(BidiCookie {
        name: cookie.name.clone(),
        value: cookie.value.clone(),
        domain: cookie.domain.clone(),
        path: cookie.path.clone(),
        http_only: cookie.http_only,
        secure: cookie.secure,
        same_site: same_site(cookie).to_ascii_lowercase(),
        expiry: expiry(cookie),
    })
}

/// `cookie` for WebKit.
pub fn to_playwright(cookie: &ChromeCookie, now: u64) -> Result<PlaywrightCookie, SkipReason> {
    check(cookie, now)?;
    Ok(PlaywrightCookie {
        name: cookie.name.clone(),
        value: cookie.value.clone(),
        domain: cookie.domain.clone(),
        path: cookie.path.clone(),
        expires: expiry(cookie)
            .and_then(|expiry| i64::try_from(expiry).ok())
            .unwrap_or(-1),
        http_only: cookie.http_only,
        secure: cookie.secure && !is_loopback(&cookie.domain),
        same_site: same_site(cookie),
    })
}

/// Whole seconds since the epoch; `None` for a session cookie.
fn expiry(cookie: &ChromeCookie) -> Option<u64> {
    if cookie.session || !cookie.expires.is_finite() || cookie.expires <= 0.0 {
        return None;
    }
    // Finite and positive: the cast saturates at u64::MAX, far past any real expiry.
    Some(cookie.expires.floor() as u64)
}

/// `Strict`, `Lax` or `None`; `Lax` when the site set none or something else.
fn same_site(cookie: &ChromeCookie) -> &'static str {
    match cookie.same_site.as_deref() {
        Some(value) if value.eq_ignore_ascii_case("strict") => "Strict",
        Some(value) if value.eq_ignore_ascii_case("none") => "None",
        _ => "Lax",
    }
}

/// `localhost` (and its subdomains), `127.0.0.1` and `[::1]`: hosts Chromium counts as secure.
pub fn is_loopback(domain: &str) -> bool {
    let host = domain.trim_start_matches('.').to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host == "127.0.0.1"
        || host == "::1"
        || host == "[::1]"
}

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;
