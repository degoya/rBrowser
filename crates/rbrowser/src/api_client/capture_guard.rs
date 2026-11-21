//! The guards of a capture the w31 security review asked for (RB-096-08): a secret in the path
//! becomes a variable like a query value ([`Names::path`]), and a request that holds a `{{` of
//! the page - one the capture did not make - is refused ([`foreign_template`]): on send it would
//! resolve `{{$secret x}}`, `{{$processEnv RB_SECRET_X}}` or another host's private value and
//! hand it to the captured host.

use rb_api_model::http_parse::form_unescape;
use rb_api_model::{ApiRequest, Body};

use super::{Names, Variable, is_secret_param};

/// The shortest path segment that may read as random (a token, a key, a hash).
const MIN_RANDOM_CHARS: usize = 20;
/// The bits per character from which a segment of lower case and digits reads as random.
const RANDOM_BITS: f64 = 4.0;
/// The longest segment taken as a plain word after a secret's name (`/token/refresh`).
const MAX_PLAIN_WORD: usize = 16;
/// Path words whose next segment is a link's secret (`/reset/<token>`, `/invite/<code>`).
const LINK_WORDS: &[&str] = &[
    "activate",
    "confirm",
    "hook",
    "hooks",
    "invitation",
    "invitations",
    "invite",
    "invites",
    "magic",
    "magic-link",
    "password-reset",
    "reset",
    "reset-password",
    "share",
    "unsubscribe",
    "verify",
    "webhook",
    "webhooks",
];

/// Whether path segment `word` names a secret that the next segment holds.
fn names_secret(word: &str) -> bool {
    let word = form_unescape(word);
    is_secret_param(&word)
        || LINK_WORDS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(&word))
}

/// A word of lower case letters and `-`, short: `/token/refresh` keeps its `refresh`.
fn is_plain_word(segment: &str) -> bool {
    segment.len() <= MAX_PLAIN_WORD && segment.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

/// The Shannon entropy of `text`, in bits per character.
fn bits(text: &str) -> f64 {
    let mut counts = std::collections::BTreeMap::new();
    for c in text.chars() {
        *counts.entry(c).or_insert(0_u32) += 1;
    }
    let total = f64::from(u32::try_from(text.chars().count()).unwrap_or(u32::MAX));
    counts
        .values()
        .map(|&count| {
            let share = f64::from(count) / total;
            -share * share.log2()
        })
        .sum()
}

/// Whether `segment` looks like a token, a key or a hash: [`MIN_RANDOM_CHARS`] or more of
/// base64url, hex and `.:=~+`, and hex with letters and digits, or upper and lower case and
/// digits, or one run of lower case and digits with [`RANDOM_BITS`] per character.
fn looks_random(segment: &str) -> bool {
    if segment.chars().count() < MIN_RANDOM_CHARS
        || !segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:=~+".contains(c))
    {
        return false;
    }
    let any = |test: fn(&char) -> bool| segment.chars().any(|c| test(&c));
    let (upper, lower, digit) = (
        any(char::is_ascii_uppercase),
        any(char::is_ascii_lowercase),
        any(char::is_ascii_digit),
    );
    let hex = segment.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        && digit
        && any(char::is_ascii_alphabetic);
    let one_run = segment.chars().all(|c| c.is_ascii_alphanumeric());
    hex || (upper && lower && digit) || (one_run && digit && bits(segment) >= RANDOM_BITS)
}

impl Names {
    /// `path` (from its first `/` on, without the query) with every secret segment a variable:
    /// the one after a secret's name or a link word, unless a plain word, and every one that
    /// [`looks_random`]. A variable is named after the plain segment before it, else `path`.
    pub(super) fn path(&mut self, path: &str) -> String {
        let mut out: Vec<String> = Vec::new();
        let mut previous: Option<&str> = None;
        for segment in path.split('/') {
            let after_secret = previous.is_some_and(names_secret) && !is_plain_word(segment);
            if !segment.is_empty() && (after_secret || looks_random(segment)) {
                let wanted = previous.filter(|word| !word.is_empty()).unwrap_or("path");
                out.push(self.variable(wanted, segment));
                previous = None;
            } else {
                out.push(segment.to_owned());
                previous = Some(segment);
            }
        }
        out.join("/")
    }
}

/// Whether `text` holds a `{{` that does not open one of `own` exactly (`{{name}}`).
fn foreign(text: &str, own: &[Variable]) -> bool {
    let mut rest = text;
    while let Some(open) = rest.find("{{") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            return true;
        };
        if !own.iter().any(|variable| variable.name == after[..close]) {
            return true;
        }
        rest = &after[close + 2..];
    }
    false
}

/// Whether `request` or a value of `own` holds a `{{` the capture did not make: every text the
/// sender resolves (`super::super::resolve`) is looked at, after the capture's own changes. A
/// capture sets no auth, so the auth is not looked at.
pub(super) fn foreign_template(request: &ApiRequest, own: &[Variable]) -> bool {
    let mut texts: Vec<&str> = vec![&request.url];
    for header in &request.headers {
        texts.extend([header.name.as_str(), header.value.as_str()]);
    }
    match &request.body {
        Body::None => {}
        Body::Json { text } | Body::Raw { text } => texts.push(text),
        Body::Binary { path } => texts.push(path),
        Body::GraphQl { query, variables } => texts.extend([query.as_str(), variables.as_str()]),
        Body::Form { fields } => {
            for field in fields {
                texts.extend([field.name.as_str(), field.value.as_str()]);
            }
        }
        Body::Multipart { parts } => {
            for part in parts {
                texts.extend([part.name.as_str(), part.value.as_str()]);
                texts.extend(part.file.as_deref());
            }
        }
    }
    texts.iter().any(|text| foreign(text, own))
        || own
            .iter()
            .filter_map(|variable| variable.value.as_deref())
            .any(|value| value.contains("{{"))
}

#[cfg(test)]
#[path = "capture_guard_tests.rs"]
mod tests;
