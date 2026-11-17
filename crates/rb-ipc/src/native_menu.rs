//! The rows of a native menu the shell asks for (RB-090-41): the toolbar's pulldowns - the
//! theme, the environment, the layout preset, the bookmarks, the overflow - name a button's
//! rectangle and their rows (`nativeMenu`), the core shows them as a native menu painted in
//! rBrowser's theme in front of the views and answers with the row chosen once the menu closed
//! (`nativeMenuClosed`). A `<select>`'s list was Chromium's popup, which followed the theme only
//! in part and could stand out of the window.
//!
//! RB-090-63: a row can be a radio choice and carry its shortcut's text (`accel`), and a
//! submenu holds rows of its own, so the always-visible main menu (`≡`) and the toolbar's
//! split buttons are menus with structure, as a native menu bar's are.

use serde::{Deserialize, Serialize};

/// One row of a native menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NativeMenuRow {
    /// A row that can be chosen; `id` comes back in `nativeMenuClosed`.
    Item {
        /// What the shell knows the row by.
        id: String,
        /// Its text.
        label: String,
        /// A check row: checked or not; `None` is a plain row.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checked: Option<bool>,
        /// One of a set of choices (`checked` the one in effect), drawn as a radio row.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        radio: bool,
        /// The command's shortcut in rb-commands' text form (`Ctrl+Shift+1`,
        /// `Ctrl+Shift+Space L G`), shown beside the label.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        accel: Option<String>,
        /// Shown, but cannot be chosen.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        disabled: bool,
    },
    /// The title of the rows after it (a group of bookmarks, a block of the overflow menu).
    Heading {
        /// Its text.
        label: String,
    },
    /// A line between rows.
    Separator,
    /// A row that opens the rows under it (RB-090-63).
    Submenu {
        /// Its text.
        label: String,
        /// The rows it opens.
        rows: Vec<NativeMenuRow>,
    },
}

#[cfg(test)]
#[path = "native_menu_tests.rs"]
mod tests;
