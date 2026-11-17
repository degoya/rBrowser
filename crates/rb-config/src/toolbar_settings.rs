//! `ui.toolbar` of the user settings (RB-080-52): what the toolbar does with what does not fit
//! into a narrow window, and whether it shows the panels' switch (RB-080-79; off by default since
//! RB-090-10) and the portal's button (RB-090-62). Settings > General > Appearance. The theme and
//! the breakpoint bar are switched in the main menu (RB-090-63); a file that still sets the
//! former `footerToggle` or `themeToggle` loads, the key unread.
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
    /// Shows the switch of the plugin panels at the right end of the toolbar (RB-080-79): it
    /// opens the docked panels and folds them to their icon rail again. Off by default
    /// (RB-090-10): the panels open from their rail beside the views, the commands and
    /// `Leader P Z`.
    pub panels_toggle: bool,
    /// Shows the portal's button in the toolbar (RB-090-62): off, a click starts the portal and
    /// shows a pairing QR code; on, a menu with the QR code, the devices, a device to disconnect
    /// and the stop, the number of paired devices on the button. Off, the portal still runs from
    /// the Portal panel, the command palette and `Leader S P`.
    pub portal_button: bool,
}

impl Default for ToolbarSettings {
    fn default() -> Self {
        Self {
            overflow: ToolbarOverflow::default(),
            panels_toggle: false,
            portal_button: true,
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
