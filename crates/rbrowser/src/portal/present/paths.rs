//! The paths a viewer may load (RB-096-05): `portal.presentation.allowPaths` - empty, the shown
//! environment's start page and everything below it - without `portal.presentation.denyPaths`.
//! A refused path answers 404 from the portal itself and never reaches the dev server.
//!
//! Every path is read in one normal form before it is compared: percent-decoded, ASCII lower
//! case, empty segments folded. A path that a server could read as another file is refused
//! outright: an encoded slash or backslash, a backslash, a NUL or another control character,
//! a `.` or `..` segment (a browser resolves those before it sends a request), an 8.3 short
//! name (`GIT~1`, which a Windows dev server may read as `.git`), bytes that are no UTF-8. An allowed path covers its segments (`/shop` allows `/shop/cart`, not
//! `/shopping`); a denied path is a plain prefix (`/.env` denies `/.env.local` too).

use rb_util::url::origin_of;

use crate::portal::origins::device_path;

/// What a presentation lets a viewer load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathRules {
    allow: Vec<String>,
    deny: Vec<String>,
}

impl PathRules {
    /// The rules of `allow` (empty: `base`, the start page's path) and `deny`; a rule that is no
    /// path (no leading `/`, a refused form) is dropped.
    #[must_use]
    pub fn new(base: &str, allow: &[String], deny: &[String]) -> Self {
        let read = |rules: &[String]| -> Vec<String> {
            rules
                .iter()
                .filter_map(|rule| normalize(rule.trim()))
                .collect()
        };
        let mut allowed = read(allow);
        if allowed.is_empty() {
            allowed = normalize(base).into_iter().collect();
        }
        Self {
            allow: allowed,
            deny: read(deny),
        }
    }

    /// Whether a viewer may load `path` (a request's path, without its query).
    #[must_use]
    pub fn allows(&self, path: &str) -> bool {
        let Some(path) = normalize(path) else {
            return false;
        };
        let denied = self.deny.iter().any(|rule| path.starts_with(rule.as_str()));
        let allowed = self
            .allow
            .iter()
            .any(|rule| rule == "/" || path == *rule || path.starts_with(&format!("{rule}/")));
        allowed && !denied
    }
}

/// `path` in the normal form of the module, without a trailing slash (`/` stays `/`); `None`
/// for a form that is refused (a short-name segment among them).
#[must_use]
pub fn normalize(path: &str) -> Option<String> {
    if !path.starts_with('/') {
        return None;
    }
    let decoded = percent_decode(path)?;
    if decoded.chars().any(|c| c == '\\' || c.is_control()) {
        return None;
    }
    let mut segments = Vec::new();
    for segment in decoded.split('/') {
        match segment {
            "" => {}
            "." | ".." => return None,
            other if rb_util::win_name::is_short_name(other) => return None,
            other => segments.push(other.to_ascii_lowercase()),
        }
    }
    Some(format!("/{}", segments.join("/")))
}

/// `text` percent-decoded; `None` for an encoded `/`, `\` or NUL, a broken escape, or bytes that
/// are no UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte != b'%' {
            out.push(byte);
            index += 1;
            continue;
        }
        let high = hex_value(*bytes.get(index + 1)?)?;
        let low = hex_value(*bytes.get(index + 2)?)?;
        let decoded = high * 16 + low;
        if matches!(decoded, b'/' | b'\\' | 0) {
            return None;
        }
        out.push(decoded);
        index += 3;
    }
    String::from_utf8(out).ok()
}

fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

/// The start page's path: that of the environment whose origin the page is on (`/shop/` of
/// `http://localhost:5173/shop/`), without query and fragment; `/` without one.
#[must_use]
pub fn base_path<'a>(
    environments: impl IntoIterator<Item = &'a str>,
    page: Option<&str>,
) -> String {
    let Some(origin) = page.and_then(origin_of) else {
        return "/".to_owned();
    };
    environments
        .into_iter()
        .find(|url| origin_of(url).as_deref() == Some(origin.as_str()))
        .and_then(device_path)
        .map(|path| {
            path.split(['?', '#'])
                .next()
                .filter(|path| !path.is_empty())
                .unwrap_or("/")
                .to_owned()
        })
        .unwrap_or_else(|| "/".to_owned())
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
