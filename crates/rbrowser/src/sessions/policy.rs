//! The SyncHub's submit hook for sessions (RB-020-07 on RB-020-02): an
//! [`rb_sync::SubmitPolicy`] over [`super::submit_views`]. With `sync.leaderPerSession` the
//! first synced view of every other session than the leader's posts a synced submit itself
//! (its own cookies); the others follow the leader by URL. The hub asks it only while
//! `sync.leaderOnlySubmit` is on.

use std::sync::{Arc, Weak};

use rb_engine::{ViewHost as _, ViewId, ViewInfo};
use rb_sync::SubmitPolicy;

use super::{SubmitRule, submit_views};
use crate::control::Core;
use crate::host::Host;

/// The views of `targets` that submit by themselves when `leader` submits: per
/// [`submit_views`] over the views in grid order (`views`, with their sessions), the leader
/// left out. `per_session` is `sync.leaderPerSession`.
pub fn submitters(
    leader: ViewId,
    targets: &[ViewId],
    views: &[ViewInfo],
    per_session: bool,
) -> Vec<ViewId> {
    let session_of = |view: ViewId| {
        views
            .iter()
            .find(|info| info.id == view)
            .map_or("default", |info| info.session.as_str())
    };
    let mut synced: Vec<(ViewId, &str)> = vec![(leader, session_of(leader))];
    synced.extend(
        views
            .iter()
            .filter(|info| targets.contains(&info.id))
            .map(|info| (info.id, info.session.as_str())),
    );
    let rule = SubmitRule {
        leader_only: true,
        per_session,
    };
    submit_views(leader, &synced, rule)
        .into_iter()
        .filter(|view| *view != leader)
        .collect()
}

/// The policy of a window: the views' sessions and `sync.leaderPerSession` read at each submit.
pub struct SessionPolicy {
    host: Weak<Host>,
}

impl SessionPolicy {
    /// The policy of `host`'s window.
    pub fn new(host: &Arc<Host>) -> Self {
        Self {
            host: Arc::downgrade(host),
        }
    }
}

impl SubmitPolicy for SessionPolicy {
    fn submitters(&self, leader: ViewId, targets: &[ViewId]) -> Vec<ViewId> {
        let Some(host) = self.host.upgrade() else {
            return Vec::new();
        };
        let per_session = host.config().sync.leader_per_session;
        submitters(leader, targets, &host.views(), per_session)
    }
}

/// Plugs the policy into the window's SyncHub; after `sync_hub::on_start`. Spawns nothing.
pub fn on_start(core: &Core) {
    if let Some(sync) = crate::sync_hub::for_host(core.host()) {
        sync.set_submit_policy(Arc::new(SessionPolicy::new(core.host())));
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
