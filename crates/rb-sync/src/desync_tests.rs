use super::*;
use crate::event::{MouseButton, MouseData, SyncEvent};
use crate::switches::tests::target;

fn click(path: &str) -> SyncEvent {
    SyncEvent::Click(MouseData {
        button: MouseButton::Left,
        detail: 1,
        modifiers: 0,
        target: target(path),
    })
}

#[test]
fn a_view_is_desynced_with_reason_selector_and_event() {
    let mut table = DesyncTable::default();
    assert!(table.mark(ViewId(2), DesyncReason::NotFound, &click("nav > a")));
    let desync = table.get(ViewId(2)).expect("desynced");
    assert_eq!(desync.reason, DesyncReason::NotFound);
    assert_eq!(desync.selector.as_deref(), Some("nav > a"));
    assert_eq!(desync.event, "click");
    assert!(table.is_desynced(ViewId(2)));
    assert!(!table.is_desynced(ViewId(3)));
}

#[test]
fn the_first_reason_stays_until_the_view_is_cleared() {
    let mut table = DesyncTable::default();
    table.mark(ViewId(2), DesyncReason::NotVisible, &click("a"));
    assert!(!table.mark(ViewId(2), DesyncReason::Timeout, &click("b")));
    assert_eq!(
        table.get(ViewId(2)).map(|desync| desync.reason),
        Some(DesyncReason::NotVisible)
    );
    assert!(table.clear(ViewId(2)));
    assert!(!table.clear(ViewId(2)), "nothing left to clear");
}

#[test]
fn the_list_is_in_view_order_and_gone_views_are_dropped() {
    let mut table = DesyncTable::default();
    table.mark(ViewId(5), DesyncReason::Timeout, &click("a"));
    table.mark(ViewId(1), DesyncReason::Failed, &click("b"));
    let views: Vec<ViewId> = table.list().iter().map(|desync| desync.view).collect();
    assert_eq!(views, vec![ViewId(1), ViewId(5)]);
    table.retain(&[ViewId(5)]);
    assert_eq!(table.list().len(), 1);
}

#[test]
fn the_reason_serialises_in_camel_case() {
    let json = serde_json::to_value(ViewDesync {
        view: ViewId(3),
        reason: DesyncReason::NotFound,
        selector: None,
        event: "click".into(),
    })
    .expect("serialises");
    assert_eq!(
        json,
        serde_json::json!({ "view": 3, "reason": "notFound", "event": "click" })
    );
}
