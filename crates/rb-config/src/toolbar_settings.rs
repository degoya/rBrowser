//! `ui.toolbar` of the user settings (RB-080-52): what the toolbar does with what does not fit
//! into a narrow window, and whether it shows the breakpoint bar's switch (RB-080-60), the theme's
//! menu (RB-080-95, RB-090-10) and the panels' switch (RB-080-79; off by default since
//! RB-090-10). Settings > General > Appearance.
//!
//! The panels' switch went from on to off by default (RB-090-10). A settings file holds only the
//! keys a person set (Settings, `settings_set` and the commands write the one key that changed,
//! a reset removes it), so whoever kept the old default has no `panelsToggle` and follows the
//! new one; a stored `true` is a choice of one's own and stays.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `ui.toolbar`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolbarSettings {
    /// What the toolbar does with the controls a narrow window has no room for: `menu` moves
    /// them into the "More" menu at its right end, `wrap` breaks the toolbar into a second row
    /// and the views move down. The address bar and back, forward and reload always stay.
    pub overflow: ToolbarOverflow,
    /// Shows the switch of the breakpoint bar under every view (formerly the footer) in the
    /// toolbar (RB-080-60). Off, the breakpoint bar is still switched in Settings, the view
    /// menus, the page's context menu and with `Leader O B`.
    pub footer_toggle: bool,
    /// Shows the menu of rBrowser's own theme in the toolbar (RB-080-95, RB-090-10): a sun and a
    /// moon in one with a mark for the theme in effect, and the choice of system, light or dark -
    /// the shell, the panels and the windows, never the pages in the views. Off, the theme is
    /// still set in Settings, the command palette and with `Leader V 0`.
    pub theme_toggle: bool,
    /// Shows the switch of the plugin panels at the right end of the toolbar (RB-080-79): it
    /// opens the docked panels and folds them to their icon rail again. Off by default
    /// (RB-090-10): the panels open from their rail beside the views, the commands and
    /// `Leader P Z`.
    pub panels_toggle: bool,
}

impl Default for ToolbarSettings {
    fn default() -> Self {
        Self {
            overflow: ToolbarOverflow::default(),
            footer_toggle: true,
            theme_toggle: true,
            panels_toggle: false,
        }
    }
}

/// `ui.toolbar.overflow`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ToolbarOverflow {
    /// One row; what does not fit is in the overflow menu.
    #[default]
    Menu,
    /// Up to two rows; only what a second row does not hold either is in the overflow menu.
    Wrap,
}

impl ToolbarOverflow {
    /// The most rows the toolbar may take.
    #[must_use]
    pub fn max_rows(self) -> u32 {
        match self {
            ToolbarOverflow::Menu => 1,
            ToolbarOverflow::Wrap => 2,
        }
    }
}

#[cfg(test)]
#[path = "toolbar_settings_tests.rs"]
mod tests;
