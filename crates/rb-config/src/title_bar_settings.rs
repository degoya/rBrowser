//! `ui.titleBar` of the user settings (RB-094-19): whether the main window's title shows
//! rBrowser's version and the credit line after the project's name and the product. Settings >
//! General > Appearance; both on by default, a change applies at once. The other windows - the
//! external engines', the settings window, the engine comparison - keep their titles.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `ui.titleBar`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct TitleBarSettings {
    /// The main window's title shows rBrowser's version after its name, as About shows it.
    pub show_version: bool,
    /// The main window's title ends in the credit line "made with Love by Alexander
    /// Herling/DEGOYA.digital".
    pub show_credit: bool,
}

impl Default for TitleBarSettings {
    fn default() -> Self {
        Self {
            show_version: true,
            show_credit: true,
        }
    }
}

#[cfg(test)]
#[path = "title_bar_settings_tests.rs"]
mod tests;
