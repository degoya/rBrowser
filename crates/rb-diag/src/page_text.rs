//! Masking what a web page writes - console texts, script and request URLs - before the host
//! hands it on: to a plugin granted `console-logs` (the `$console` feed) and to MCP's
//! `console_logs`, `network_requests` and `errors` (review finding L1 of Charge 6).
//!
//! [`crate::redact::redact`] first - registered secrets, a secret's value after its name
//! (`access_token=`, `password=`), `Bearer` values, the user info of an address - then the URL
//! parameters that carry a credential under a name the log rules do not know: the OAuth code
//! and verifier, signed-URL signatures and credentials, SAML messages. A parameter counts only
//! after `?`, `&`, `;` or `#`, so `exit code=1` in prose stays. As with the log rules, text with
//! nothing to hide comes back byte for byte and masking twice equals masking once.

use crate::redact::{PLACEHOLDER, redact};

/// URL parameter names (lower-case) whose value is a credential.
const URL_SECRET_PARAMS: &[&str] = &[
    "assertion",
    "code",
    "jwt",
    "oauth_verifier",
    "samlrequest",
    "samlresponse",
    "sig",
    "signature",
    "ticket",
    "x-amz-credential",
    "x-amz-signature",
    "x-goog-credential",
    "x-goog-signature",
];

/// `text` with every secret [`crate::redact::redact`] finds and every credential URL parameter
/// masked.
#[must_use]
pub fn redact_page_text(text: &str) -> String {
    mask_url_params(&redact(text))
}

fn mask_url_params(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while let Some(offset) = text[index..].find(['?', '&', ';', '#']) {
        let name_start = index + offset + 1;
        out.push_str(&text[index..name_start]);
        index = name_start;
        let name_len = text[name_start..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || "_-.".contains(c)))
            .unwrap_or(text.len() - name_start);
        let name = &text[name_start..name_start + name_len];
        let after_name = name_start + name_len;
        if name_len == 0
            || !text[after_name..].starts_with('=')
            || !URL_SECRET_PARAMS.contains(&name.to_ascii_lowercase().as_str())
        {
            continue;
        }
        let value_start = after_name + 1;
        let value_end = text[value_start..]
            .find(|c: char| c.is_whitespace() || "&#;\"'<>".contains(c))
            .map_or(text.len(), |end| value_start + end);
        let value = &text[value_start..value_end];
        out.push_str(&text[name_start..value_start]);
        out.push_str(if value.is_empty() { value } else { PLACEHOLDER });
        index = value_end;
    }
    out.push_str(&text[index..]);
    out
}

#[cfg(test)]
#[path = "page_text_tests.rs"]
mod tests;
