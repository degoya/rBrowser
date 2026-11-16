//! Masking of secrets in URLs and headers (RB-080-37): one list of the header names and query
//! parameter names that carry a cookie, a token or a password, for the app (MCP answers, the
//! log) and the plugins (session files) alike. A masked value is [`MASK`].

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

/// Query parameter names whose values are secrets (case-blind).
pub const SENSITIVE_PARAMS: &[&str] = &[
    "token",
    "access_token",
    "refresh_token",
    "id_token",
    "auth_token",
    "api_key",
    "apikey",
    "key",
    "secret",
    "client_secret",
    "password",
    "passwd",
    "pwd",
    "auth",
    "session",
    "sessionid",
    "session_id",
    "sid",
    "signature",
    "sig",
    "code",
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

/// `url` with the sensitive query parameters masked; the fragment stays.
#[must_use]
pub fn mask_url(url: &str) -> String {
    let Some((base, rest)) = url.split_once('?') else {
        return url.to_owned();
    };
    let (query, fragment) = rest
        .split_once('#')
        .map_or((rest, None), |(query, fragment)| (query, Some(fragment)));
    let masked: Vec<String> = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((name, _)) if sensitive_param(name) => format!("{name}={MASK}"),
            _ => pair.to_owned(),
        })
        .collect();
    let mut out = format!("{base}?{}", masked.join("&"));
    if let Some(fragment) = fragment {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

#[cfg(test)]
#[path = "mask_tests.rs"]
mod tests;
