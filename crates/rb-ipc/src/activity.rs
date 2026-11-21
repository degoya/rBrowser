//! The activity bar's part of the protocol (RB-090-67): the activities it has entries for, its
//! state in the snapshot - where the core made room for it, the entries folded into their
//! toolbar indicator, the recording just stopped - and the running video recording, the one
//! activity that published nothing to the shell before. The other activities come from what the
//! shell already gets: `recorder`, `sync.desynced`, `timeControl` (RB-094-07),
//! `pluginProgress` (RB-093-01) and `forcedStates` (RB-094-16) in the snapshot, `flowRun`,
//! `checksProgress` and `portal`.

use rb_config::ActivityBarPosition;
use rb_engine::ViewId;
use serde::{Deserialize, Serialize};

/// An activity with an entry in the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Activity {
    /// A video recording.
    Video,
    /// A recording of steps.
    Recorder,
    /// A flow run, a recording played back included.
    Flows,
    /// A check run, Lighthouse included.
    Checks,
    /// The running portal.
    Portal,
    /// Views out of sync.
    Desync,
    /// The page clock stopped or set, or animations held (RB-094-07).
    Time,
    /// A plugin call that reports its progress (`host.report-progress`, RB-093-01).
    Plugins,
    /// Pseudo-class states forced on an element of a view (RB-094-16).
    ForcedStates,
}

/// The activity bar in the snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityBarState {
    /// Where the core made room for the bar: the shell page there draws it. Absent while it has
    /// none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<ActivityBarPosition>,
    /// The entries folded into their toolbar indicator by their close button, until their
    /// activity ends; sorted, each once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collapsed: Vec<Activity>,
    /// The recording that just stopped: the recorder's entry offers to play it until it is
    /// played, closed or another recording starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded: Option<String>,
}

impl ActivityBarState {
    /// Nothing to tell: no room, nothing folded, nothing to play.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.place.is_none() && self.collapsed.is_empty() && self.recorded.is_none()
    }

    /// Folds `activity` into its indicator (`collapsed`) or opens it again; whether that
    /// changed anything.
    pub fn set_collapsed(&mut self, activity: Activity, collapsed: bool) -> bool {
        let found = self.collapsed.binary_search(&activity);
        match (found, collapsed) {
            (Err(at), true) => {
                self.collapsed.insert(at, activity);
                true
            }
            (Ok(at), false) => {
                self.collapsed.remove(at);
                true
            }
            _ => false,
        }
    }
}

/// A video recording that runs: what the bar and the toolbar show of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoActivity {
    /// The views recorded.
    pub views: Vec<ViewId>,
    /// When it started, ms since the epoch: the shell counts the time from it.
    pub started_at_ms: u64,
    /// It stops by itself after this many seconds (of recorded time).
    pub max_duration_sec: u64,
    /// While paused (RB-090-68): when the pause began, ms since the epoch - the shell's time
    /// stands still at it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_at_ms: Option<u64>,
    /// The ms of the pauses that ended: the shell's time leaves them out.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub paused_ms: u64,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
