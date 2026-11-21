//! Secret values masked in what leaves the core: an answer to MCP, the history, a log line. The
//! value is replaced as it is, JSON-escaped, form-escaped (a `.http` body line) and form-encoded
//! (a query key, a sent form body), the longest first, so a server that
//! echoes a token (`/anything`, an error page) does not hand it on.

use crate::http_file::{form_encode, form_escape};

/// What a secret becomes.
pub const MASK: &str = "***";
/// A shorter value is not masked: it would mask ordinary text everywhere.
pub const MIN_SECRET_CHARS: usize = 3;

/// The forms of `secrets` to look for, the longest first.
fn forms(secrets: &[String]) -> Vec<String> {
    let mut forms: Vec<String> = secrets
        .iter()
        .filter(|secret| secret.chars().count() >= MIN_SECRET_CHARS)
        .flat_map(|secret| {
            let json = serde_json::to_string(secret).unwrap_or_default();
            let json = json
                .strip_prefix('"')
                .and_then(|json| json.strip_suffix('"'))
                .unwrap_or(&json)
                .to_owned();
            [
                secret.clone(),
                json,
                form_escape(secret),
                form_encode(secret),
            ]
        })
        .collect();
    forms.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    forms.dedup();
    forms
}

/// `text` with every form of `secrets` replaced by [`MASK`].
#[must_use]
pub fn mask(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_owned();
    for form in forms(secrets) {
        if out.contains(&form) {
            out = out.replace(&form, MASK);
        }
    }
    out
}

/// `value` with [`mask`] applied to every string and key in it.
#[must_use]
pub fn mask_json(value: &serde_json::Value, secrets: &[String]) -> serde_json::Value {
    if secrets.is_empty() {
        return value.clone();
    }
    match value {
        serde_json::Value::String(text) => serde_json::Value::String(mask(text, secrets)),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(|item| mask_json(item, secrets)).collect())
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(key, item)| (mask(key, secrets), mask_json(item, secrets)))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[cfg(test)]
#[path = "mask_tests.rs"]
mod tests;
