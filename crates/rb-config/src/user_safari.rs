//! `engines.safari`: the real Safari on macOS over `safaridriver` (RB-095-03). On Windows and
//! Linux there is no Safari: the keys are kept and do nothing there.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `engines.safari`: Safari over `safaridriver`, macOS only (RB-095-03).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SafariSettings {
    /// Breakpoints on Safari open in a Safari window on macOS (the installed Safari, driven by
    /// `safaridriver`); off, they stay with Chromium. Without macOS Safari is not available.
    pub enabled: bool,
    /// Safari Technology Preview instead of Safari, when it is installed (its own
    /// `safaridriver`, which needs `--enable` once as well).
    pub technology_preview: bool,
}

impl Default for SafariSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            technology_preview: false,
        }
    }
}

#[cfg(test)]
#[path = "user_safari_tests.rs"]
mod tests;
