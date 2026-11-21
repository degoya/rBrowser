//! The cookies a document sets, read for their attributes only: a cookie's **value is dropped
//! where it is read** - [`Cookie`] has no field for it - so it reaches no finding, log line or
//! answer.
//!
//! Two sources: the `Set-Cookie` headers of the document's answer ([`parse_set_cookie`]) and the
//! cookies the view holds for the page's URL (CDP `Network.getCookies`, [`from_cdp`]). Both are
//! foreign text: at most [`MAX_COOKIES`] cookies and [`MAX_ATTRIBUTES`] attributes a line are
//! read, names cut to [`MAX_NAME_CHARS`] characters without control characters.

use serde_json::Value;

use crate::date;
use crate::headers::shown;

/// The most cookies read of one answer.
pub const MAX_COOKIES: usize = 64;
/// The most attributes read of one `Set-Cookie` line.
pub const MAX_ATTRIBUTES: usize = 32;
/// The longest cookie name kept, in characters.
pub const MAX_NAME_CHARS: usize = 64;
/// The longest `Path` kept, in characters.
pub const MAX_PATH_CHARS: usize = 256;

/// The `SameSite` attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

impl SameSite {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "strict" => Some(Self::Strict),
            "lax" => Some(Self::Lax),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    /// The attribute's value as written in a header.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "Strict",
            Self::Lax => "Lax",
            Self::None => "None",
        }
    }
}

/// One cookie's name and attributes; never its value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub secure: bool,
    pub http_only: bool,
    /// `None` when unset or unknown (the browser's default applies).
    pub same_site: Option<SameSite>,
    /// Whether a `Domain` attribute widens it beyond the host.
    pub domain: bool,
    pub path: Option<String>,
    /// Whether the line deletes the cookie (`Max-Age` 0 or less, `Expires` in the past): it
    /// removes the name, it is no live cookie to hold to the rules.
    pub deleted: bool,
}

/// Before 2000: an `Expires` that old deletes a cookie when the answer has no `Date`.
const LONG_AGO: i64 = 946_684_800;
/// The most digits of a `Max-Age` read.
const MAX_AGE_DIGITS: usize = 18;

/// The `Max-Age` of `value` (RFC 6265, 5.2.2); `None` when it is no number.
fn max_age(value: &str) -> Option<i64> {
    let value = value.trim();
    let (negative, digits) = value
        .strip_prefix('-')
        .map_or((false, value), |digits| (true, digits));
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number: i64 = digits
        .get(..digits.len().min(MAX_AGE_DIGITS))
        .and_then(|digits| digits.parse().ok())?;
    Some(if negative { -number } else { number })
}

/// The cookie of one `Set-Cookie` line; `None` for a line without a name. An `Expires` is held
/// to the answer's `Date` (Unix seconds), without one to the year 2000.
#[must_use]
pub fn parse_set_cookie(line: &str) -> Option<Cookie> {
    parse_set_cookie_at(line, None)
}

/// [`parse_set_cookie`] with the answer's `Date` as `now`.
#[must_use]
pub fn parse_set_cookie_at(line: &str, now: Option<i64>) -> Option<Cookie> {
    let mut parts = line.split(';');
    let (name, _value) = parts.next()?.split_once('=')?;
    let name = shown(name.trim(), MAX_NAME_CHARS);
    if name.is_empty() {
        return None;
    }
    let mut cookie = Cookie {
        name,
        ..Cookie::default()
    };
    let (mut max_age_seen, mut expires) = (None, None);
    for attribute in parts.take(MAX_ATTRIBUTES) {
        let (key, value) = attribute.split_once('=').unwrap_or((attribute, ""));
        match key.trim().to_ascii_lowercase().as_str() {
            "secure" => cookie.secure = true,
            "httponly" => cookie.http_only = true,
            "samesite" => cookie.same_site = SameSite::parse(value),
            "domain" => cookie.domain = !value.trim().is_empty(),
            "path" => cookie.path = Some(shown(value.trim(), MAX_PATH_CHARS)),
            "max-age" => max_age_seen = max_age(value).or(max_age_seen),
            "expires" => expires = date::parse(value).or(expires),
            _ => {}
        }
    }
    // `Max-Age` wins over `Expires` (RFC 6265, 5.3).
    cookie.deleted = match (max_age_seen, expires) {
        (Some(age), _) => age <= 0,
        (None, Some(at)) => at <= now.unwrap_or(LONG_AGO),
        (None, None) => false,
    };
    Some(cookie)
}

/// The cookies of a CDP `Network.getCookies` answer; anything malformed is left out.
#[must_use]
pub fn from_cdp(answer: &str) -> Vec<Cookie> {
    let Ok(value) = serde_json::from_str::<Value>(answer) else {
        return Vec::new();
    };
    let Some(list) = value.get("cookies").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|entry| {
            let name = shown(entry.get("name")?.as_str()?.trim(), MAX_NAME_CHARS);
            if name.is_empty() {
                return None;
            }
            let flag = |key: &str| entry.get(key).and_then(Value::as_bool).unwrap_or(false);
            Some(Cookie {
                name,
                secure: flag("secure"),
                http_only: flag("httpOnly"),
                same_site: entry
                    .get("sameSite")
                    .and_then(Value::as_str)
                    .and_then(SameSite::parse),
                // CDP names a host-only cookie's host, a domain cookie's domain with a dot.
                domain: entry
                    .get("domain")
                    .and_then(Value::as_str)
                    .is_some_and(|domain| domain.starts_with('.')),
                path: entry
                    .get("path")
                    .and_then(Value::as_str)
                    .map(|path| shown(path, MAX_PATH_CHARS)),
                deleted: false,
            })
        })
        .take(MAX_COOKIES)
        .collect()
}

/// `earlier` with the cookies of a later answer `later` applied in order: a name set again
/// replaces the earlier one, a deleted one removes it; at most [`MAX_COOKIES`].
#[must_use]
pub fn apply(earlier: Vec<Cookie>, later: Vec<Cookie>) -> Vec<Cookie> {
    let mut cookies = earlier;
    for cookie in later {
        cookies.retain(|seen| seen.name != cookie.name);
        if !cookie.deleted {
            cookies.push(cookie);
        }
    }
    cookies.truncate(MAX_COOKIES);
    cookies
}

/// `first` and then the cookies of `second` whose name `first` does not have, at most
/// [`MAX_COOKIES`].
#[must_use]
pub fn merge(first: Vec<Cookie>, second: Vec<Cookie>) -> Vec<Cookie> {
    let mut merged = first;
    for cookie in second {
        if !merged.iter().any(|seen| seen.name == cookie.name) {
            merged.push(cookie);
        }
    }
    merged.truncate(MAX_COOKIES);
    merged
}

/// What one cookie lacks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CookieAudit {
    /// The attributes missing, of `Secure`, `HttpOnly` and `SameSite`, in that order.
    pub missing: Vec<&'static str>,
    /// The prefix (`__Secure-`, `__Host-`) whose rules the cookie breaks.
    pub prefix: Option<&'static str>,
}

impl CookieAudit {
    /// Whether the cookie is fine.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.prefix.is_none()
    }
}

fn has_prefix(name: &str, prefix: &str) -> bool {
    name.get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

/// `cookie` held to the rules; `https` is the page's scheme. `Secure` is asked for over HTTPS,
/// and always with `SameSite=None` (which a browser rejects without it).
#[must_use]
pub fn audit(cookie: &Cookie, https: bool) -> CookieAudit {
    let mut missing = Vec::new();
    if !cookie.secure && (https || cookie.same_site == Some(SameSite::None)) {
        missing.push("Secure");
    }
    if !cookie.http_only {
        missing.push("HttpOnly");
    }
    if cookie.same_site.is_none() {
        missing.push("SameSite");
    }
    let prefix = if has_prefix(&cookie.name, "__Host-") {
        let broken = !cookie.secure || cookie.domain || cookie.path.as_deref() != Some("/");
        broken.then_some("__Host-")
    } else if has_prefix(&cookie.name, "__Secure-") {
        (!cookie.secure).then_some("__Secure-")
    } else {
        None
    };
    CookieAudit { missing, prefix }
}

/// The attributes of `cookie` that the environment comparison holds side by side: `Secure`,
/// `HttpOnly`, `SameSite`; empty for none. Never the value, the lifetime or the domain (which
/// differ between environments by nature).
#[must_use]
pub fn summary(cookie: &Cookie) -> String {
    let mut parts: Vec<String> = Vec::new();
    if cookie.secure {
        parts.push("Secure".to_owned());
    }
    if cookie.http_only {
        parts.push("HttpOnly".to_owned());
    }
    if let Some(same_site) = cookie.same_site {
        parts.push(format!("SameSite={}", same_site.as_str()));
    }
    parts.join("; ")
}

#[cfg(test)]
#[path = "cookies_tests.rs"]
mod tests;
