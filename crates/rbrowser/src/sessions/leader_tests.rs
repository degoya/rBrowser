use super::*;

const A: ViewId = ViewId(1);
const B: ViewId = ViewId(2);
const C: ViewId = ViewId(3);
const D: ViewId = ViewId(4);

fn synced() -> Vec<(ViewId, &'static str)> {
    vec![(A, "default"), (B, "default"), (C, "admin"), (D, "admin")]
}

#[test]
fn one_post_per_session_by_default() {
    let rule = SubmitRule {
        leader_only: true,
        per_session: true,
    };
    assert_eq!(submit_views(A, &synced(), rule), [A, C]);
    // The leader of a session is the first synced view of it; the leader's own session keeps it.
    assert_eq!(submit_views(D, &synced(), rule), [D, A]);
}

#[test]
fn without_the_rule_only_the_leader_posts() {
    let rule = SubmitRule {
        leader_only: true,
        per_session: false,
    };
    assert_eq!(submit_views(A, &synced(), rule), [A]);
}

#[test]
fn without_leader_only_submit_every_view_posts() {
    let rule = SubmitRule {
        leader_only: false,
        per_session: true,
    };
    assert_eq!(submit_views(B, &synced(), rule), [B, A, C, D]);
}

#[test]
fn a_single_session_has_a_single_poster() {
    let rule = SubmitRule {
        leader_only: true,
        per_session: true,
    };
    let synced = [(A, "default"), (B, "default")];
    assert_eq!(submit_views(B, &synced, rule), [B]);
}

#[test]
fn a_leader_outside_the_synced_views_still_posts() {
    let rule = SubmitRule {
        leader_only: true,
        per_session: true,
    };
    let synced = [(B, "default"), (C, "admin")];
    assert_eq!(submit_views(A, &synced, rule), [A, B, C]);
}
