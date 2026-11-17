use std::time::Duration;

use rb_config::ScrollMode;
use rb_engine::ViewId;
use serde_json::json;

use super::*;
use crate::replay::mcp::InputStatus;
use crate::scroll_sync::fake::ScrollPort;

fn args(value: serde_json::Value) -> ScrollArgs {
    serde_json::from_value(value).expect("arguments")
}

#[test]
fn top_and_bottom_are_the_exact_ends() {
    let (top, require) = scroll_request(&args(json!({ "to": "top" }))).expect("valid");
    assert_eq!((top.y, top.x), (0.0, 0.0));
    assert!(require.is_none() && top.anchor.is_none() && top.container.is_none());
    let (bottom, _) = scroll_request(&args(json!({ "to": "bottom" }))).expect("valid");
    assert_eq!(bottom.y, bottom.max_y);
    assert!(
        bottom.max_y > 1.0,
        "the agent's end is within 1 of the range"
    );
}

#[test]
fn an_element_is_anchored_at_the_top_edge_and_must_be_found() {
    let (data, require) =
        scroll_request(&args(json!({ "element": "#s5", "container": "#sidebar" }))).expect("valid");
    let anchor = data.anchor.expect("the element as the anchor");
    assert_eq!(anchor.offset.y, 0.0);
    assert_eq!(require, Some(anchor));
    assert!(
        data.y > 0.0 && data.y < data.max_y,
        "inside the range, so the anchor counts"
    );
    let container = data.container.expect("the container");
    assert_eq!(
        serde_json::to_value(&container.element).expect("json"),
        json!({ "kind": "css", "path": "#sidebar" })
    );
}

#[test]
fn a_percentage_is_the_share_of_the_range() {
    let (data, _) =
        scroll_request(&args(json!({ "percentY": 25, "percentX": 100 }))).expect("valid");
    assert_eq!(data.y / data.max_y, 0.25);
    assert_eq!(data.x, data.max_x);
}

#[test]
fn exactly_one_destination_is_taken_and_every_value_is_checked() {
    for bad in [
        json!({}),
        json!({ "to": "top", "element": "#a" }),
        json!({ "to": "top", "percentY": 10 }),
        json!({ "percentY": 101 }),
        json!({ "percentX": -1 }),
        json!({ "element": "  " }),
        json!({ "element": "a".repeat(3000) }),
        json!({ "to": "top", "container": "" }),
    ] {
        assert!(
            matches!(
                scroll_request(&args(bad.clone())),
                Err(CommandError::InvalidArgs(_))
            ),
            "{bad}"
        );
    }
    assert!(serde_json::from_value::<ScrollArgs>(json!({ "to": "middle" })).is_err());
    assert!(serde_json::from_value::<ScrollArgs>(json!({ "y": 10 })).is_err());
}

#[tokio::test]
async fn every_view_is_scrolled_side_by_side_with_its_own_status() {
    let port = ScrollPort::new(ScrollMode::Wheel, json!({ "status": "done" }));
    let (data, require) = scroll_request(&args(json!({ "to": "bottom" }))).expect("valid");
    let answer = scroll_views(
        &port,
        &[ViewId(1), ViewId(2)],
        &data,
        require.as_ref(),
        Duration::from_secs(1),
    )
    .await;
    assert_eq!(answer.views.len(), 2);
    assert!(
        answer
            .views
            .iter()
            .all(|view| view.status == InputStatus::Done)
    );
    // MCP sets the position in every mode: the wheel mode is for the sync of a wheel.
    assert_eq!(port.worlds().len(), 2);
    assert!(port.sends().is_empty());
}

#[tokio::test]
async fn a_missing_element_or_container_is_not_found() {
    let port = ScrollPort::new(ScrollMode::Anchor, json!({ "status": "not_found" }));
    let (data, require) = scroll_request(&args(json!({ "element": "#nope" }))).expect("valid");
    let answer = scroll_views(
        &port,
        &[ViewId(3)],
        &data,
        require.as_ref(),
        Duration::from_secs(1),
    )
    .await;
    assert_eq!(answer.views[0].status, InputStatus::NotFound);
}
