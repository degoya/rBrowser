//! `ui.activityBar` of the user settings (RB-090-67): the activity bar, a strip of the shell
//! that keeps its room (`always`, RB-094-13) or is there only while something runs - a video, a
//! recording, a flow run, a check run, the portal, views out of sync, a changed clock, a
//! plugin's long call, forced pseudo-class states (RB-094-16) - with the running activity's
//! controls. Where it stands, whether it is drawn at all or the toolbar's indicators carry the
//! activities alone, and which activities have an entry in it. Settings > General >
//! Appearance, beside the toolbar's switches.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `ui.activityBar`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ActivityBarSettings {
    /// Where the bar stands while something runs: `top`, below the toolbar, or `bottom`, above
    /// the status bar. The views give it its height and take it back when the last activity
    /// ends.
    pub position: ActivityBarPosition,
    /// `always` keeps the bar's room, empty while nothing runs, so the views never move; `bar`
    /// shows the bar only while something runs; `indicator` draws no bar and leaves each
    /// activity to its compact indicator in the toolbar.
    pub mode: ActivityBarMode,
    /// Which activities have an entry in the bar; one switched off shows only its compact
    /// indicator in the toolbar.
    pub show: ActivityBarShow,
}

/// `ui.activityBar.position`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ActivityBarPosition {
    /// Below the toolbar.
    #[default]
    Top,
    /// Above the status bar.
    Bottom,
}

/// `ui.activityBar.mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ActivityBarMode {
    /// The bar always, empty while nothing runs: the views keep their place (RB-094-13).
    #[default]
    Always,
    /// The bar with the running activities' controls, only while something runs.
    Bar,
    /// No bar; the toolbar's compact indicators only.
    Indicator,
}

/// `ui.activityBar.show`: an entry in the bar per activity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ActivityBarShow {
    /// A video recording: its time, the views recorded and Stop.
    pub video: bool,
    /// A recording of steps: its steps, Pause or Resume, Stop, Add check and Play last.
    pub recorder: bool,
    /// A flow run, a recording played back included: the views through, Cancel and Results.
    pub flows: bool,
    /// A check run, Lighthouse included: its progress and Cancel.
    pub checks: bool,
    /// The running portal: its devices, a new QR code and Disconnect.
    pub portal: bool,
    /// Views out of sync: how many, and Resync.
    pub desync: bool,
    /// The page clock or the animations changed (RB-094-07): the time and rate, Step, Real
    /// time and 100 %.
    pub time: bool,
    /// A plugin call that reports its progress (RB-093-01): the plugin, how far it is and
    /// Cancel when the plugin has a cancel command.
    pub plugins: bool,
    /// Pseudo-class states forced on an element (RB-094-16): the states, the views and Reset.
    pub forced_states: bool,
}

impl Default for ActivityBarShow {
    fn default() -> Self {
        Self {
            video: true,
            recorder: true,
            flows: true,
            checks: true,
            portal: true,
            desync: true,
            time: true,
            plugins: true,
            forced_states: true,
        }
    }
}

#[cfg(test)]
#[path = "activity_bar_settings_tests.rs"]
mod tests;
