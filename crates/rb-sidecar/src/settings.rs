//! The settings the sidecar reads (Engines > Node runtime, Engines > WebKit). The fields live in
//! rb-config's `EngineSettings` (`engines.nodePath` since contract 0.1; the other two in
//! `schema/parts/sidecar.json`); the binary hands them over as [`SidecarSettings`].

use std::time::Duration;

/// `engines.nodePath`: a Node runtime of one's own instead of the bundled one.
pub const KEY_NODE_PATH: &str = "engines.nodePath";
/// `engines.nodeRpcTimeoutMs`: how long a request waits for its answer.
pub const KEY_RPC_TIMEOUT: &str = "engines.nodeRpcTimeoutMs";
/// `engines.playwrightBrowsersPath`: where the Playwright browsers go.
pub const KEY_BROWSERS_PATH: &str = "engines.playwrightBrowsersPath";
/// `engines.lighthouseBrowserPath`: the Chromium-based browser Lighthouse runs in.
pub const KEY_LIGHTHOUSE_BROWSER: &str = "engines.lighthouseBrowserPath";

/// The default of [`KEY_RPC_TIMEOUT`].
pub const DEFAULT_RPC_TIMEOUT_MS: u32 = 30_000;
/// The bounds of [`KEY_RPC_TIMEOUT`] (the schema holds the same).
pub const RPC_TIMEOUT_RANGE_MS: std::ops::RangeInclusive<u32> = 1_000..=600_000;

/// What the sidecar reads from the user settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidecarSettings {
    /// `engines.nodePath`; `None` or empty takes the bundled runtime.
    pub node_path: Option<String>,
    /// `engines.nodeRpcTimeoutMs`.
    pub rpc_timeout_ms: u32,
    /// `engines.playwrightBrowsersPath`; `None` or empty takes
    /// `%LOCALAPPDATA%\rBrowser\ms-playwright`.
    pub browsers_path: Option<String>,
    /// `engines.lighthouseBrowserPath`; `None` or empty lets the sidecar find Playwright's
    /// Chromium, an installed Chrome or Edge (RB-080-58).
    pub lighthouse_browser_path: Option<String>,
}

impl Default for SidecarSettings {
    fn default() -> Self {
        Self {
            node_path: None,
            rpc_timeout_ms: DEFAULT_RPC_TIMEOUT_MS,
            browsers_path: None,
            lighthouse_browser_path: None,
        }
    }
}

impl SidecarSettings {
    /// The request timeout, clamped to [`RPC_TIMEOUT_RANGE_MS`].
    pub fn rpc_timeout(&self) -> Duration {
        let ms = self
            .rpc_timeout_ms
            .clamp(*RPC_TIMEOUT_RANGE_MS.start(), *RPC_TIMEOUT_RANGE_MS.end());
        Duration::from_millis(u64::from(ms))
    }
}

/// A path setting with surrounding blanks removed; `None` when empty.
pub(crate) fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
