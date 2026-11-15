//! The hub's public types ([`crate::hub`]): where an event goes, why it goes nowhere, a
//! follow's navigation, the error and the status the shell and MCP see.

use rb_engine::ViewId;

use crate::desync::ViewDesync;
use crate::replay::ReplayRequest;
use crate::switches::{SyncKind, SyncSwitches};

/// Why an event goes nowhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// Sync is off.
    Disabled,
    /// The event's view is not the active one.
    NotActive,
    /// The event's view is excluded from sync.
    Excluded,
    /// The kind of the event is not synced.
    KindOff(SyncKind),
    /// Part of the click `Leader S N` skips.
    SkippedClick,
    /// No other view can take it.
    NoTargets,
    /// Without leader-only submit every view submits through its own replayed input.
    SubmitFollowsInput,
}

/// Where an event goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    /// Nowhere.
    Skip(SkipReason),
    /// Replayed into each request's view.
    Replay(Vec<ReplayRequest>),
    /// A leader-only submit: the followers load the leader's URL once its navigation commits.
    Follow {
        /// The view that submitted.
        leader: ViewId,
        /// The views that follow by URL.
        followers: Vec<ViewId>,
    },
}

impl Route {
    /// Whether the event is replayed somewhere.
    pub fn is_replay(&self) -> bool {
        matches!(self, Route::Replay(_))
    }
}

/// A URL the hub has a view load (a follow).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Navigation {
    /// The view.
    pub view: ViewId,
    /// The URL.
    pub url: String,
}

/// A request named a view the hub does not know.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HubError {
    /// No view has this id.
    #[error("unknown view {0}")]
    UnknownView(ViewId),
}

/// What the shell and MCP show of the sync.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HubStatus {
    /// The switches in effect.
    pub switches: SyncSwitches,
    /// The views excluded from sync, by id.
    pub excluded: Vec<ViewId>,
    /// The desynced views, by id.
    pub desynced: Vec<ViewDesync>,
    /// `Leader S N` is armed.
    pub skip_next_click: bool,
}
