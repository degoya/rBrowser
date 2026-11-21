//! Secret fields (RB-050-01): a password field (the agent marks its events `sensitive`) or a
//! field named in `automation.recorder.secretFields` is never recorded with its value; its
//! `fill` holds the placeholder `{{secret:<name>}}` instead, the name taken from the field.
//!
//! A `secretFields` entry names a field the way the recorder sees it: its CSS path as the
//! recorder's selectors show it (`#otp`, `form > input:nth-of-type(2)`; the path or its last
//! steps after a ` > `), `[data-testid="otp"]`, or the field's accessible name (`One-time code`,
//! any case). The page is not asked: matching runs on what the event carries.
//!
//! A URL's tokens are no steps' business either: the value of a known token parameter (the one
//! list `rb_util::mask`) or of a parameter a `secretFields` entry names, in the query or the
//! fragment, is recorded as `********`, and the user info (`user:password@`) is dropped
//! ([`mask_url`]). Such a value is single-use or bound to a session, so a replay gains nothing
//! from it.

use crate::event::{ElementSelector, ElementTarget};

/// The longest secret name.
const MAX_NAME: usize = 32;
/// The name of a secret whose field has none.
const FALLBACK_NAME: &str = "secret";

/// `url` without its user info and with the value of every token parameter in its query and
/// fragment as `********`: one of `rb_util::mask`'s list (RB-094-10) or named by an entry of
/// `extra` (`secretFields`, any case). The user info is dropped, not masked: a replay meets the
/// site's login (Basic auth answers it from the person's store) instead of a wrong name.
pub fn mask_url(url: &str, extra: &[String]) -> String {
    rb_util::mask::mask_url_with(
        &rb_util::url::without_user_info(url),
        extra,
        crate::forms::MASK,
    )
}

/// Whether the field `target` is secret: a password field (`sensitive`) or one of `extra`.
pub fn is_secret(sensitive: bool, target: &ElementTarget, extra: &[String]) -> bool {
    sensitive
        || extra
            .iter()
            .any(|entry| matches_entry(target, entry.trim()))
}

/// The name of the secret in field `target`: its test id, accessible name or the id at the end
/// of its CSS path, as `[a-z0-9_-]`; `secret` when it has none.
pub fn secret_name(target: &ElementTarget) -> String {
    let raw = match &target.element {
        ElementSelector::Testid { testid, .. } => testid.clone(),
        ElementSelector::Role { name, path, .. } if name.trim().is_empty() => last_id(path),
        ElementSelector::Role { name, .. } => name.clone(),
        ElementSelector::Text { path, .. } | ElementSelector::Css { path } => last_id(path),
    };
    let name = slug(&raw);
    if name.is_empty() {
        FALLBACK_NAME.to_owned()
    } else {
        name
    }
}

fn matches_entry(target: &ElementTarget, entry: &str) -> bool {
    if entry.is_empty() {
        return false;
    }
    let (path, testid, name) = match &target.element {
        ElementSelector::Testid { testid, path, .. } => (path, Some(testid), None),
        ElementSelector::Role { name, path, .. } => (path, None, Some(name)),
        ElementSelector::Text { path, .. } | ElementSelector::Css { path } => (path, None, None),
    };
    let path_matches = path == entry
        || path.ends_with(&format!(" > {entry}"))
        || path.ends_with(&format!(" {entry}"));
    let testid_matches = testid.is_some_and(|testid| {
        entry == format!("[data-testid=\"{testid}\"]")
            || entry == format!("[data-testid='{testid}']")
    });
    let name_matches = name.is_some_and(|name| name.trim().eq_ignore_ascii_case(entry));
    path_matches || testid_matches || name_matches
}

/// The id of the CSS path's last step (`form > input#otp` -> `otp`); empty without one.
fn last_id(path: &str) -> String {
    let last = path
        .rsplit(['>', ' '])
        .find(|part| !part.is_empty())
        .unwrap_or("");
    last.split_once('#')
        .map(|(_, id)| id.split(['.', ':', '[']).next().unwrap_or("").to_owned())
        .unwrap_or_default()
}

fn slug(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-')
        .chars()
        .take(MAX_NAME)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned()
}

#[cfg(test)]
#[path = "secrets_tests.rs"]
mod tests;
