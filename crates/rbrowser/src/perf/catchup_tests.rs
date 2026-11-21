use rb_sync::{ElementSelector, ElementTarget, EventMessage, Offset, ScrollData, SyncEvent};

use super::*;

fn scroll(y: f64, container: Option<ElementTarget>) -> ScrollData {
    ScrollData {
        container,
        anchor: None,
        x: 0.0,
        y,
        max_x: 0.0,
        max_y: 2000.0,
    }
}

fn message(top: bool, data: ScrollData) -> EventMessage {
    EventMessage {
        v: rb_sync::PROTOCOL_VERSION,
        top,
        frame: None,
        seq: 1,
        time: 0,
        path: Vec::new(),
        event: SyncEvent::Scroll(data),
    }
}

fn facts(view_url: &str, leader_url: &str) -> CatchUpFacts {
    CatchUpFacts {
        view: ViewId(2),
        view_url: view_url.to_owned(),
        leader: Some((ViewId(1), leader_url.to_owned())),
        enabled: true,
        navigation: true,
        scroll: true,
        excluded: false,
        decoupled: false,
        desynced: false,
    }
}

fn memo_with(y: f64) -> ScrollMemo {
    let mut memo = ScrollMemo::default();
    memo.observe(ViewId(1), &message(true, scroll(y, None)));
    memo
}

#[test]
fn a_view_on_another_url_loads_the_leaders() {
    let memo = memo_with(400.0);
    assert_eq!(
        plan(&facts("https://a.test/old", "https://a.test/new"), &memo),
        Some(CatchUpStep::Resync)
    );
}

#[test]
fn a_view_on_the_leaders_url_takes_its_last_document_scroll() {
    let memo = memo_with(400.0);
    assert_eq!(
        plan(&facts("https://a.test/", "https://a.test/"), &memo),
        Some(CatchUpStep::Scroll {
            source: ViewId(1),
            data: Box::new(scroll(400.0, None))
        })
    );
}

#[test]
fn only_the_document_scroll_of_the_top_frame_is_remembered() {
    let mut memo = memo_with(400.0);
    let container = ElementTarget {
        hosts: Vec::new(),
        element: ElementSelector::Css {
            path: "#side".to_owned(),
        },
        offset: Offset { x: 0.0, y: 0.0 },
    };
    memo.observe(ViewId(1), &message(true, scroll(9.0, Some(container))));
    memo.observe(ViewId(1), &message(false, scroll(7.0, None)));
    assert_eq!(memo.get(ViewId(1)), Some(&scroll(400.0, None)));
    memo.observe(ViewId(1), &message(true, scroll(500.0, None)));
    assert_eq!(memo.get(ViewId(1)), Some(&scroll(500.0, None)));
}

#[test]
fn a_navigation_forgets_the_scroll_of_the_document_left() {
    let mut memo = memo_with(400.0);
    memo.navigated(ViewId(1));
    assert_eq!(
        plan(&facts("https://a.test/", "https://a.test/"), &memo),
        None
    );
}

#[test]
fn nothing_is_caught_up_without_sync_or_for_an_excluded_desynced_or_leading_view() {
    let memo = memo_with(400.0);
    let base = facts("https://a.test/old", "https://a.test/new");
    for facts in [
        CatchUpFacts {
            enabled: false,
            ..base.clone()
        },
        CatchUpFacts {
            excluded: true,
            ..base.clone()
        },
        CatchUpFacts {
            desynced: true,
            ..base.clone()
        },
        CatchUpFacts {
            leader: None,
            ..base.clone()
        },
        CatchUpFacts {
            leader: Some((ViewId(2), "https://a.test/new".to_owned())),
            ..base.clone()
        },
    ] {
        assert_eq!(plan(&facts, &memo), None, "{facts:?}");
    }
}

#[test]
fn navigation_off_or_decoupled_leaves_the_url_and_scroll_off_leaves_the_position() {
    let memo = memo_with(400.0);
    let other = facts("https://a.test/old", "https://a.test/new");
    let off = CatchUpFacts {
        navigation: false,
        ..other.clone()
    };
    assert_eq!(plan(&off, &memo), None);
    let decoupled = CatchUpFacts {
        decoupled: true,
        ..other
    };
    assert_eq!(plan(&decoupled, &memo), None);
    let same = CatchUpFacts {
        scroll: false,
        ..facts("https://a.test/", "https://a.test/")
    };
    assert_eq!(plan(&same, &memo), None);
}

#[test]
fn an_empty_leader_url_is_nothing_to_load() {
    let memo = ScrollMemo::default();
    assert_eq!(plan(&facts("https://a.test/", ""), &memo), None);
}

#[test]
fn a_closed_view_is_forgotten() {
    let mut memo = memo_with(400.0);
    memo.retain(&[ViewId(2)]);
    assert_eq!(memo.get(ViewId(1)), None);
}
