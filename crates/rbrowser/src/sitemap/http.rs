//! The outgoing side the plugins' HTTP transport (`plugins::services::http`) shares since sitemap
//! capture, the first plugin that fetched: the time one request may take and the request headers
//! a plugin may set. Only `Accept` and `Accept-Language` go out, each at most 1 KiB and without a
//! control character - no cookie, no authorization, nothing else a plugin names.

use std::time::Duration;

use rb_plugin_types::HttpRequest;

/// How long one request may take.
pub const TIMEOUT: Duration = Duration::from_secs(30);
/// The request headers a plugin may set.
pub const REQUEST_HEADERS: [&str; 2] = ["accept", "accept-language"];

/// The headers of `request` that go out.
pub fn outgoing(request: &HttpRequest) -> Vec<(String, String)> {
    request
        .headers
        .iter()
        .filter(|(name, value)| {
            REQUEST_HEADERS.contains(&name.to_ascii_lowercase().as_str())
                && value.len() <= 1024
                && !value.chars().any(char::is_control)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
