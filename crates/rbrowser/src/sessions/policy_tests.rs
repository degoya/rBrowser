use rb_engine::{EngineKind, ViewId, ViewInfo};

use super::submitters;

fn view(id: u32, session: &str) -> ViewInfo {
    ViewInfo {
        id: ViewId(id),
        label: format!("v{id}"),
        breakpoint: None,
        cell_breakpoint: None,
        engine: EngineKind::Chromium,
        width: 375,
        height: 812,
        dpr: 1.0,
        scale: 1.0,
        url: String::new(),
        title: String::new(),
        loading: false,
        session: session.to_owned(),
        cell: None,
    }
}

fn grid() -> Vec<ViewInfo> {
    vec![
        view(1, "default"),
        view(2, "default"),
        view(3, "admin"),
        view(4, "admin"),
    ]
}

#[test]
fn the_first_target_of_every_other_session_submits_itself() {
    let targets = [ViewId(2), ViewId(3), ViewId(4)];
    assert_eq!(submitters(ViewId(1), &targets, &grid(), true), [ViewId(3)]);
    // From an admin leader, the default session's first synced view posts.
    let targets = [ViewId(1), ViewId(2), ViewId(3)];
    assert_eq!(submitters(ViewId(4), &targets, &grid(), true), [ViewId(1)]);
}

#[test]
fn without_leader_per_session_only_the_leader_submits() {
    let targets = [ViewId(2), ViewId(3), ViewId(4)];
    assert!(submitters(ViewId(1), &targets, &grid(), false).is_empty());
}

#[test]
fn a_session_with_no_target_view_has_no_submitter() {
    // View 3 and 4 are excluded or desynced: the hub does not name them.
    assert!(submitters(ViewId(1), &[ViewId(2)], &grid(), true).is_empty());
}
