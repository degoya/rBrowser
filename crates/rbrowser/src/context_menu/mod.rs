//! The page's context menu (RB-080-43): below Chromium's own entries, rBrowser's block for the
//! clicked view - Inspect element, the DevTools check, the submenus named and ordered as the main
//! menu's (View, Layout, Sync, Emulation, Environments, Capture, Checks, Tools, Flows, Panels;
//! RB-090-92), then "Do not sync the next click", "Pop out" / "Back into the grid" and, once
//! RB-080-41 defines `view.remove`, "Remove view". Every entry
//! runs a command of the one registry; what cannot run now is disabled, not hidden;
//! `ui.pageContextMenu` off leaves Chromium's menu and "Do not sync the next click".
//!
//! | Part | Module |
//! | --- | --- |
//! | The entries, their texts, ids and enabled flags, built from a [`model::MenuState`] | [`model`] |
//! | The submenus grouped as the main menu groups them, its keys and order (RB-090-92) | [`model::groups`] |
//! | The Emulation submenu: the toolbar's switches for the clicked view (RB-080-49) | [`emulation`] |
//! | The Force state submenu: pseudo-class states for the clicked element (RB-090-53) | [`force_state`] |
//! | The Recorder and Run flow submenus: record, check steps, flows (RB-090-60) | [`recorder`] |
//! | Sync, Sessions and the plugins' Tools entries of the clicked view (RB-090-63) | [`view_entries`] |
//! | The state read from the core when the menu opens | [`state`] |
//! | Running the chosen entry on the core's runtime | [`run`] |
//! | The block a view showed last, for the handler that gets the choice | [`shown`] |
//!
//! The CEF side is `cef_app/context_menu.rs`: it renders the items and maps a chosen id back.

pub mod emulation;
pub mod force_state;
pub mod model;
pub mod recorder;
pub mod run;
pub mod shown;
pub mod state;
pub mod time_control;
pub mod view_entries;

pub use model::{Action, Entry, Item, Label, MenuState, build, find};
pub use run::run;
pub use shown::ViewMenu;
pub use state::state_for;
