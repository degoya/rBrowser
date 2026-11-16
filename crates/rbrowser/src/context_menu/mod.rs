//! The page's context menu (RB-080-43): below Chromium's own entries, rBrowser's block for the
//! clicked view - Inspect element, Open/Close DevTools, Screenshot, Video, Checks and Panels
//! submenus, "Footer under every view" (RB-080-60), then "Do not sync the next click", "Pop out" /
//! "Back into the grid" and, once RB-080-41 defines `view.remove`, "Remove view". Every entry
//! runs a command of the one registry; what cannot run now is disabled, not hidden;
//! `ui.pageContextMenu` off leaves Chromium's menu and "Do not sync the next click".
//!
//! | Part | Module |
//! | --- | --- |
//! | The entries, their texts, ids and enabled flags, built from a [`model::MenuState`] | [`model`] |
//! | The Emulation submenu: the toolbar's switches for the clicked view (RB-080-49) | [`emulation`] |
//! | The Force state submenu: pseudo-class states for the clicked element (RB-090-53) | [`force_state`] |
//! | The state read from the core when the menu opens | [`state`] |
//! | Running the chosen entry on the core's runtime | [`run`] |
//! | The block a view showed last, for the handler that gets the choice | [`shown`] |
//!
//! The CEF side is `cef_app/context_menu.rs`: it renders the items and maps a chosen id back.

pub mod emulation;
pub mod force_state;
pub mod model;
pub mod run;
pub mod shown;
pub mod state;

pub use model::{Action, Entry, Item, Label, MenuState, build, find};
pub use run::run;
pub use shown::ViewMenu;
pub use state::state_for;
