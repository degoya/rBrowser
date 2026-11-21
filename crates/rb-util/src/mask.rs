//! Masking of secrets in URLs and headers (RB-080-37, RB-094-10): one list of the header names
//! and URL parameter names that carry a cookie, a token or a password, for the app (MCP answers,
//! the log, recordings, bug reports) and the plugins (session files) alike. A masked value is
//! [`MASK`].

/// The mask.
pub const MASK: &str = "***";

/// Header names whose values are secrets (case-blind).
pub const SENSITIVE_HEADERS: &[&str] = &[
    "cookie",
    "set-cookie",
    "set-cookie2",
    "authorization",
    "proxy-authorization",
    "www-authenticate",
    "x-api-key",
    "x-auth-token",
    "x-csrf-token",
    "x-xsrf-token",
    "x-amz-security-token",
];

/// Query and fragment parameter names whose values are secrets (case-blind): the one list
/// (RB-094-10). The log rules (`rb_diag::redact`), the page texts (`rb_diag::page_text`), the
/// recorder (`rb_sync::recorder::secrets`) and the bug report build on it; a test of each holds
/// its own narrower list inside this one. OAuth's `state` is a nonce, not a credential.
pub const SENSITIVE_PARAMS: &[&str] = &[
    // Tokens and keys.
    "token",
    "access_token",
    "refresh_token",
    "id_token",
    "auth_token",
    "authtoken",
    "api_key",
    "api-key",
    "apikey",
    "key",
    "access_key",
    "private_key",
    "secret",
    "client_secret",
    "jwt",
    // Passwords.
    "password",
    "passwd",
    "passphrase",
    "pwd",
    "auth",
    // Sessions.
    "session",
    "sessionid",
    "session_id",
    "sid",
    // OAuth, CAS and SAML.
    "code",
    "oauth_verifier",
    "ticket",
    "assertion",
    "samlrequest",
    "samlresponse",
    // A device's UDID identifies the device (`breakpoints[].iosDevice`, RB-095-04).
    "iosdevice",
    // Signed URLs.
    "signature",
    "sig",
    "x-amz-signature",
    "x-amz-credential",
    "x-amz-security-token",
    "x-goog-signature",
    "x-goog-credential",
];

/// Whether header `name` carries a secret.
#[must_use]
pub fn sensitive_header(name: &str) -> bool {
    SENSITIVE_HEADERS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(name))
}

/// Whether query parameter `name` carries a secret.
#[must_use]
pub fn sensitive_param(name: &str) -> bool {
    SENSITIVE_PARAMS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(name))
}

/// `headers` with the sensitive values masked.
#[must_use]
pub fn mask_headers(headers: &[(String, String)]) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            if sensitive_header(name) {
                (name.clone(), MASK.to_owned())
            } else {
                (name.clone(), value.clone())
            }
        })
        .collect()
}

/// `url` with its user info and the values of its sensitive query and fragment parameters
/// masked: `https://***@host/cb?code=***#access_token=***`. Masking twice equals masking once.
#[must_use]
pub fn mask_url(url: &str) -> String {
    let url = match crate::url::user_info_span(url) {
        Some((start, end)) => format!("{}{MASK}@{}", &url[..start], &url[end..]),
        None => url.to_owned(),
    };
    mask_url_with(&url, &[], MASK)
}

/// `url` with the value of every sensitive parameter - one of [`SENSITIVE_PARAMS`] or named by
/// an entry of `extra` (trimmed, any case) - in its query and its fragment (a hash route's own
/// query included; `&` and `;` separate) replaced by `mask`. The user info stays: a caller
/// masks or drops it as its output needs.
#[must_use]
pub fn mask_url_with(url: &str, extra: &[String], mask: &str) -> String {
    let Some(start) = url.find(['?', '#']) else {
        return url.to_owned();
    };
    let (head, tail) = url.split_at(start);
    let mut out = String::with_capacity(url.len());
    out.push_str(head);
    for piece in tail.split_inclusive(['?', '#', '&', ';']) {
        let (body, separator) = match piece.char_indices().last() {
            Some((at, '?' | '#' | '&' | ';')) => piece.split_at(at),
            _ => (piece, ""),
        };
        match body.split_once('=') {
            Some((name, _)) if secret_param(name, extra) => {
                out.push_str(name);
                out.push('=');
                out.push_str(mask);
            }
            _ => out.push_str(body),
        }
        out.push_str(separator);
    }
    out
}

fn secret_param(name: &str, extra: &[String]) -> bool {
    !name.is_empty()
        && (sensitive_param(name)
            || extra
                .iter()
                .any(|entry| entry.trim().eq_ignore_ascii_case(name)))
}

#[cfg(test)]
#[path = "mask_tests.rs"]
mod tests;
