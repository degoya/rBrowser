//! `panels` of the user settings (RB-080-57, Plugins > Installed): where the plugin panels
//! show - in a window of their own or docked in the main window - how large the dock is on each
//! side, whether it is open, folded to its icon rail or hidden (RB-080-93), how it starts and
//! what its Close does, how the panels' side bar shows (RB-080-61), and how many hidden panels
//! stay loaded (RB-090-38). Read by `crates/rbrowser/src/panel_dock` and the panels page.
//!
//! `panels.dockCollapsed` (RB-080-61) became `panels.dockState`: an older file is still read
//! (`true` the rail, `false` open), the old key is never written again.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

/// `panels.keepFrames` when a file does not say.
const KEEP_FRAMES: u32 = 3;

/// `panels`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PanelSettings {
    /// Where the plugin panels show: docked `right` (the default), `left` or `bottom` in the
    /// main window, where the views make room, or `floating` in a window of their own. The dock
    /// commands and the Dock menu in the panels' header set it.
    pub dock: PanelDock,
    /// The docked panels' size per side, in DIP.
    pub dock_size: PanelDockSize,
    /// The docked panels now: `open` at their size, `rail` folded to the icon rail (the header
    /// below the views, the icon column beside them) so the views get the room, or `hidden`.
    /// The expand and collapse buttons, a panel icon in the rail, Close and
    /// `panels.dock.toggle` set it (RB-080-93).
    pub dock_state: PanelDockState,
    /// The panels' side bar: `expanded` shows icons with titles, `icons` only the icons, with
    /// the title as a tooltip (RB-080-61). A narrow dock shows only icons either way.
    pub sidebar: PanelSidebar,
    /// How the docked panels start: `rail` folded to the icon rail (the default), `hidden`, or
    /// `last` as they were left (RB-080-93). Panels in their own window start at the rail.
    pub dock_at_start: PanelDockAtStart,
    /// What Close does to the panels (the X in their header, `panels.close`, closing their own
    /// window): `rail` folds them to the icon rail (the default), `hidden` hides them
    /// (RB-080-93).
    pub close_to: PanelCloseTo,
    /// How many of the panels shown last stay loaded, hidden, beside the shown one, so
    /// switching back to one does not load it again (RB-090-38); `0` loads each panel anew.
    #[schemars(range(max = 10))]
    pub keep_frames: u32,
}

impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            dock: PanelDock::default(),
            dock_size: PanelDockSize::default(),
            dock_state: PanelDockState::default(),
            sidebar: PanelSidebar::default(),
            dock_at_start: PanelDockAtStart::default(),
            close_to: PanelCloseTo::default(),
            keep_frames: KEEP_FRAMES,
        }
    }
}

/// What a settings file holds under `panels`, the key of RB-080-61 included.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct PanelSettingsFile {
    dock: PanelDock,
    dock_size: PanelDockSize,
    dock_state: Option<PanelDockState>,
    /// RB-080-61's `panels.dockCollapsed`, read for the migration only.
    dock_collapsed: Option<bool>,
    sidebar: PanelSidebar,
    dock_at_start: PanelDockAtStart,
    close_to: PanelCloseTo,
    keep_frames: Option<u32>,
}

impl<'de> Deserialize<'de> for PanelSettings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let file = PanelSettingsFile::deserialize(deserializer)?;
        let migrated = file.dock_collapsed.map(|collapsed| {
            if collapsed {
                PanelDockState::Rail
            } else {
                PanelDockState::Open
            }
        });
        Ok(Self {
            dock: file.dock,
            dock_size: file.dock_size,
            dock_state: file.dock_state.or(migrated).unwrap_or_default(),
            sidebar: file.sidebar,
            dock_at_start: file.dock_at_start,
            close_to: file.close_to,
            keep_frames: file.keep_frames.unwrap_or(KEEP_FRAMES),
        })
    }
}

/// `panels.sidebar`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PanelSidebar {
    /// Icons with titles.
    #[default]
    Expanded,
    /// Icons only.
    Icons,
}

/// `panels.dock`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PanelDock {
    /// A window of their own.
    Floating,
    /// Left of the views.
    Left,
    /// Right of the views.
    #[default]
    Right,
    /// Below the views.
    Bottom,
}

/// `panels.dockState`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PanelDockState {
    /// Open at their size (in their own window: the window is open).
    Open,
    /// Folded to the icon rail.
    #[default]
    Rail,
    /// Hidden: no rail, no window.
    Hidden,
}

/// `panels.dockAtStart`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PanelDockAtStart {
    /// Folded to the icon rail.
    #[default]
    Rail,
    /// Hidden.
    Hidden,
    /// As they were left: open, the rail or hidden.
    Last,
}

/// `panels.closeTo`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PanelCloseTo {
    /// Folded to the icon rail.
    #[default]
    Rail,
    /// Hidden.
    Hidden,
}

/// `panels.dockSize`: the bar between the views and the panels sets the side in use. The views
/// keep at least a quarter of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PanelDockSize {
    /// The width of the panels docked left of the views, in DIP.
    #[schemars(range(min = 120, max = 4000))]
    pub left: u32,
    /// The width of the panels docked right of the views, in DIP.
    #[schemars(range(min = 120, max = 4000))]
    pub right: u32,
    /// The height of the panels docked below the views, in DIP.
    #[schemars(range(min = 120, max = 4000))]
    pub bottom: u32,
}

impl Default for PanelDockSize {
    fn default() -> Self {
        Self {
            left: 520,
            right: 520,
            bottom: 320,
        }
    }
}

#[cfg(test)]
#[path = "panel_settings_tests.rs"]
mod tests;
