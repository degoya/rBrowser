//! The address of a Lighthouse run a plugin starts (security review RB-090-06, finding 4;
//! RB-090-21). Lighthouse loads its URL in a browser of its own and answers the report with
//! screenshots, so without a limit a plugin with the `sidecar` grant - or an MCP client through
//! the Lighthouse plugin's `url` - could read loopback or LAN pages no view ever showed. A run is
//! held to the sites of the views: the URL's origin must be the origin of a view's page. A
//! person lifts the limit with the protected setting `plugins.lighthouseAnyUrl`
//! (`HostServices::lighthouse_any_url`).
//!
//! The URL is read strictly, so the host compared here is the host the browser loads: no user
//! info (`@`), no backslash (a browser ends the host there, `https://127.0.0.1\@site/`), no
//! whitespace or control character (a browser drops tabs and line breaks).

use rb_plugin_types::{Failure, FailureKind, ViewInfo, codes};
use serde_json::Value;

/// The sidecar method that starts a run and names its URL.
pub(crate) const START: &str = "lighthouse.start";

/// `plugins.error.sidecar_url_denied` unless the `url` of `params` lies on the origin of one of
/// `views`' pages.
///
/// # Errors
///
/// The refusal, naming the URL (cut to 200 characters).
pub(crate) fn check(params: &str, views: &[ViewInfo]) -> Result<(), Failure> {
    let url = serde_json::from_str::<Value>(params)
        .ok()
        .and_then(|params| params.get("url")?.as_str().map(str::to_owned))
        .unwrap_or_default();
    let allowed = origin(&url).is_some_and(|origin| {
        views
            .iter()
            .any(|view| rb_util::url::origin_of(&view.url).as_deref() == Some(origin.as_str()))
    });
    if allowed {
        Ok(())
    } else {
        Err(
            Failure::coded(FailureKind::AccessDenied, codes::SIDECAR_URL_DENIED)
                .with_param("url", url.chars().take(200).collect::<String>()),
        )
    }
}

/// The origin of `url` when every browser reads the same host from it: `None` with a backslash,
/// whitespace, a control character or user info.
fn origin(url: &str) -> Option<String> {
    if url
        .chars()
        .any(|c| c == '\\' || c.is_whitespace() || c.is_control())
    {
        return None;
    }
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return None;
    }
    rb_util::url::origin_of(url)
}

#[cfg(test)]
#[path = "lighthouse_url_tests.rs"]
mod tests;
