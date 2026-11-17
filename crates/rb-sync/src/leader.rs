//! The leader model (`sync.leaderOnlySubmit`): a form submit - and with it every non-GET
//! navigation, which only a form can start - runs only in the leader, the view that captured it
//! (the active view). The replayed input carries `block_submit`, so a replayed click on a submit
//! button or Enter in a field does not submit in the other views; once the leader's navigation
//! commits, they load its URL ([`Follow`]).
//!
//! **The session hook.** With named sessions (RB-020-07) each session has a cookie jar of its
//! own, and a view of another session than the leader's has to submit by itself. A
//! [`SubmitPolicy`] names those views; the hub replays the input into them without
//! `block_submit` and lets only the rest follow by URL. The default is [`LeaderOnly`];
//! RB-020-07 plugs its `sessions::submit_views` in with `Hub::set_submit_policy`.

use std::time::{Duration, Instant};

use rb_engine::ViewId;

/// How long the followers wait for the leader's navigation after a submit. A submit that does
/// not navigate (a script handles it) leaves nothing to follow.
pub const FOLLOW_WINDOW: Duration = Duration::from_secs(15);

/// Which views besides the leader submit a form themselves.
pub trait SubmitPolicy: Send + Sync {
    /// Of `targets` (every view the event goes to, the leader not among them), the ones that
    /// submit by themselves when `leader` submits; the rest follow the leader by URL.
    fn submitters(&self, leader: ViewId, targets: &[ViewId]) -> Vec<ViewId>;
}

/// One session: only the leader submits.
#[derive(Clone, Copy, Debug, Default)]
pub struct LeaderOnly;

impl SubmitPolicy for LeaderOnly {
    fn submitters(&self, _leader: ViewId, _targets: &[ViewId]) -> Vec<ViewId> {
        Vec::new()
    }
}

/// The leader submitted: the followers load the URL its navigation commits next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Follow {
    /// The view that submitted.
    pub leader: ViewId,
    /// The views that load its URL.
    pub followers: Vec<ViewId>,
    /// After this the submit is forgotten.
    pub deadline: Instant,
}

impl Follow {
    /// A follow of `leader` by `followers`, open for [`FOLLOW_WINDOW`] from `now`.
    pub fn new(leader: ViewId, followers: Vec<ViewId>, now: Instant) -> Self {
        Self {
            leader,
            followers,
            deadline: now + FOLLOW_WINDOW,
        }
    }

    /// Whether `view`'s navigation at `now` is the one the followers wait for.
    pub fn is_due(&self, view: ViewId, now: Instant) -> bool {
        view == self.leader && now <= self.deadline
    }

    /// Whether the window has closed at `now`.
    pub fn expired(&self, now: Instant) -> bool {
        now > self.deadline
    }
}

#[cfg(test)]
#[path = "leader_tests.rs"]
mod tests;
