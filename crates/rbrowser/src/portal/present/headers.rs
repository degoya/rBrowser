//! The headers of the presentation's traffic (RB-096-05): what a viewer's request never takes
//! to the dev server and what the dev server's answer never takes to the viewer. No cookie goes
//! either way - the presenter's session never reaches a viewer, a viewer's browser keeps none of
//! the site's - and neither do Cloudflare's own headers (the viewer's address among them) or the
//! dev server's details (`Server`, `X-Powered-By`, debug toolbars, source maps). Names are
//! compared in lower case ([`super::super::rewrite`] lowers them).

/// Request headers that stay at the portal: the cookie, Cloudflare's loop guard and every header
/// that names the viewer's address (cloudflared adds `X-Forwarded-For` beside `Cf-Connecting-Ip`).
const REQUEST: &[&str] = &[
    "cookie",
    "cdn-loop",
    "x-forwarded-for",
    "forwarded",
    "x-real-ip",
    "true-client-ip",
    "x-client-ip",
    "x-cluster-client-ip",
];
/// Response headers that stay at the portal.
const RESPONSE: &[&str] = &[
    "set-cookie",
    "server",
    "x-powered-by",
    "x-aspnet-version",
    "x-aspnetmvc-version",
    "x-debug-token",
    "x-debug-token-link",
    "x-sourcemap",
    "sourcemap",
    "x-runtime",
    "via",
];

/// Whether the request header `lower` stays at the portal.
#[must_use]
pub fn request_dropped(lower: &str) -> bool {
    REQUEST.contains(&lower) || lower.starts_with("cf-")
}

/// Whether the response header `lower` stays at the portal.
#[must_use]
pub fn response_dropped(lower: &str) -> bool {
    RESPONSE.contains(&lower)
}

#[cfg(test)]
#[path = "headers_tests.rs"]
mod tests;
