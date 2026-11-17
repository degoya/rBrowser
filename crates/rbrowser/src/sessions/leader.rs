//! The leader rule for synced form submits across sessions (RB-020-07), for the SyncHub of
//! RB-020-02 to apply.
//!
//! With `sync.leaderOnlySubmit` only the leader view sends a synced submit, so a login form
//! filled in every view is posted once. Views in another session do not share the leader's
//! cookies, so a post from the leader would not log them in: with `sync.leaderPerSession` (on
//! by default) every session has a leader of its own - the leader for its session, the first
//! synced view of each other session - and each of them posts. Without `leaderOnlySubmit`
//! every synced view posts, as before.

use rb_engine::ViewId;

/// The submit settings in effect (`sync.leaderOnlySubmit`, `sync.leaderPerSession`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubmitRule {
    /// Only a leader posts a synced submit.
    pub leader_only: bool,
    /// One leader per session instead of one in all.
    pub per_session: bool,
}

/// The views that send a submit the leader `leader` made, leader first, then grid order.
/// `synced` are the views in sync with their session, in grid order.
pub fn submit_views(leader: ViewId, synced: &[(ViewId, &str)], rule: SubmitRule) -> Vec<ViewId> {
    let mut views = vec![leader];
    if !rule.leader_only {
        views.extend(
            synced
                .iter()
                .map(|(view, _)| *view)
                .filter(|v| *v != leader),
        );
        return views;
    }
    if !rule.per_session {
        return views;
    }
    let mut covered: Vec<&str> = synced
        .iter()
        .filter(|(view, _)| *view == leader)
        .map(|(_, session)| *session)
        .collect();
    for (view, session) in synced {
        if *view != leader && !covered.contains(session) {
            covered.push(session);
            views.push(*view);
        }
    }
    views
}

#[cfg(test)]
#[path = "leader_tests.rs"]
mod tests;
