//! `layoutModes` of the user settings (RB-080-04, Layout > Further modes): the defaults of the
//! horizontal, split and canvas modes a preset does not set itself, and the minimap. Read by
//! `crates/rbrowser/src/layout`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ConfigFile, MAX_SNAP, StripHeight};

/// The smallest and largest keyboard step of the split divider.
pub const SPLIT_STEP_RANGE: (f64, f64) = (0.01, 0.25);

/// `layoutModes`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutModeSettings {
    /// Canvas: a moved view snaps to this grid in DIP (0 for none) and to the edges of views
    /// closer than it, unless the preset sets its own `snap`.
    #[schemars(range(max = 128))]
    pub snap: u32,
    /// Canvas: the minimap in the toolbar strip shows every view and the visible area; a click
    /// centres the canvas on that point.
    pub minimap: bool,
    /// Canvas: the minimap's place in the toolbar strip.
    pub minimap_position: MinimapPosition,
    /// Horizontal: the row height when the preset sets none - `"fit"` (the grid area's) or a
    /// height in DIP.
    pub horizontal_height: StripHeight,
    /// Split: how far one keyboard step moves the divider, as a share of the area (0.01 to
    /// 0.25).
    #[schemars(range(min = 0.01, max = 0.25))]
    pub split_step: f64,
    /// Canvas: Space held turns any drag on the canvas board - a view's label too - into a pan;
    /// a drag on the empty board always pans.
    pub space_pan: bool,
    /// The file "Save layout as preset" writes when the call names none: `local`
    /// (`config.local.json`, personal) or `project` (`config.json`, shared with the team).
    pub save_preset_to: PresetFile,
    /// Canvas: Ctrl + mouse wheel over the canvas board - the free area or a view's label -
    /// zooms the canvas around the pointer; the views grow and shrink with it. Off: the wheel
    /// does nothing there. Over a view the page zooms, as in every layout.
    pub wheel_zoom: bool,
    /// Canvas: the middle mouse button held and dragged pans the canvas, as Space + drag does -
    /// over a view, a label or a breakpoint bar too, where the page's autoscroll does not start
    /// then. Off gives the middle button back to the pages.
    pub middle_button_pan: bool,
    /// Canvas: a view that reaches past the edge of the grid area shows the part of its page
    /// that lies inside; only a view wholly outside is hidden and paused. Off: a view is hidden
    /// as soon as any part of it leaves the area.
    pub show_partial_views: bool,
}

/// The config file a saved layout preset goes to (`layout.save_preset`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PresetFile {
    /// `config.local.json`: personal, not checked in.
    #[default]
    Local,
    /// `config.json`: checked in, shared with the team.
    Project,
}

impl PresetFile {
    /// The config file it names.
    pub fn config_file(self) -> ConfigFile {
        match self {
            PresetFile::Local => ConfigFile::Local,
            PresetFile::Project => ConfigFile::Project,
        }
    }

    /// Its name in the config and the answers (`local`, `project`).
    pub fn name(self) -> &'static str {
        match self {
            PresetFile::Local => "local",
            PresetFile::Project => "project",
        }
    }
}

/// Where the minimap sits in the toolbar strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MinimapPosition {
    /// At the end of the toolbar, before the menus.
    #[default]
    End,
    /// At the start of the toolbar, after the logo.
    Start,
}

impl Default for LayoutModeSettings {
    fn default() -> Self {
        Self {
            snap: 8,
            minimap: true,
            minimap_position: MinimapPosition::End,
            horizontal_height: StripHeight::Fit,
            split_step: 0.05,
            space_pan: true,
            save_preset_to: PresetFile::Local,
            wheel_zoom: true,
            middle_button_pan: true,
            show_partial_views: true,
        }
    }
}

impl LayoutModeSettings {
    /// The settings with every number in its range (a hand-edited file is not validated).
    pub fn clamped(self) -> Self {
        let step = if self.split_step.is_finite() {
            self.split_step
                .clamp(SPLIT_STEP_RANGE.0, SPLIT_STEP_RANGE.1)
        } else {
            Self::default().split_step
        };
        Self {
            snap: self.snap.min(MAX_SNAP),
            split_step: step,
            ..self
        }
    }
}

#[cfg(test)]
#[path = "layout_mode_settings_tests.rs"]
mod tests;
