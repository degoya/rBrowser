//! What the page itself tells: the collector (`src/collect.js`) through `evaluate` in the
//! plugin's isolated world - the manifest links, `theme-color`, `apple-touch-icon`, the secure
//! context and the service worker registration - and the offline probe, whether the Cache
//! Storage holds the start URL. Both only read.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde::Deserialize;

/// The collector, one expression.
pub const COLLECTOR: &str = include_str!("collect.js");

/// The service worker registration of the page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Registration {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
    /// The newest worker's state (`installing`, `installed`, `activating`, `activated`,
    /// `redundant`).
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub active: bool,
}

/// The service worker as the page sees it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Worker {
    /// `navigator.serviceWorker` exists (a secure context in a browser that has it).
    pub supported: bool,
    /// A service worker controls the page.
    pub controlled: bool,
    pub registration: Option<Registration>,
    /// The error `getRegistration` threw (its name).
    pub error: Option<String>,
}

/// What the collector answers for one view.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PageState {
    pub url: String,
    pub secure: bool,
    /// The `href` of every `<link rel="manifest">`, at most four.
    pub manifests: Vec<String>,
    pub theme_color: bool,
    pub apple_touch_icon: Option<String>,
    pub sw: Worker,
}

/// The state of the page `view` shows.
///
/// # Errors
///
/// The host's failure (`not_available` for a Firefox or WebKit view), or
/// `plugins.error.bad_output` for an answer that is no state.
pub fn collect(host: &mut dyn PluginHost, view: ViewId) -> Result<PageState, Failure> {
    let answer = host.evaluate(view, COLLECTOR)?;
    serde_json::from_str::<Option<PageState>>(&answer)
        .ok()
        .flatten()
        .ok_or_else(|| {
            Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT).with_param("reason", "collect")
        })
}

/// The offline probe of `url`: whether a cache of the origin holds it (the query ignored, as
/// most service workers' cache lookups do), `null` without Cache Storage.
#[must_use]
pub fn offline_probe(url: &str) -> String {
    let literal = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_owned());
    format!(
        "(async () => {{ if (typeof caches === 'undefined') return null; \
         try {{ return !!(await caches.match({literal}, {{ ignoreSearch: true }})); }} \
         catch (error) {{ return null; }} }})()"
    )
}

/// Whether the Cache Storage of `view`'s page holds `url`; `None` when it cannot tell.
pub fn cached(host: &mut dyn PluginHost, view: ViewId, url: &str) -> Option<bool> {
    let answer = host.evaluate(view, &offline_probe(url)).ok()?;
    serde_json::from_str::<Option<bool>>(&answer).ok().flatten()
}

#[cfg(test)]
#[path = "collect_tests.rs"]
mod tests;
