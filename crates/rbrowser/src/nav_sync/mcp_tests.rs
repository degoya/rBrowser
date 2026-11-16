use serde_json::json;

use super::*;
use crate::nav_sync::couple;
use crate::nav_sync::tests::started;
use crate::sync_hub::tests::setup;

#[tokio::test(flavor = "multi_thread")]
async fn views_list_shows_whether_a_view_follows_navigation() {
    let before = setup();
    assert!(view_navigation(before.core.host(), ViewId(1)).sync_navigation);

    let setup = started(&["mobile"]);
    let host = setup.core.host();
    assert!(view_navigation(host, ViewId(1)).sync_navigation);
    assert!(!view_navigation(host, ViewId(3)).sync_navigation);
    couple(host, ViewId(3), true).expect("a view");
    assert!(view_navigation(host, ViewId(3)).sync_navigation);
    assert_eq!(
        serde_json::to_value(view_navigation(host, ViewId(3))).expect("serializes"),
        json!({ "syncNavigation": true })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_set_decouples_one_view_s_navigation() {
    let setup = started(&[]);
    let state = crate::sync_hub::mcp::sync_set(
        &setup.core,
        &json!({ "target": 3, "feature": "navigation", "enabled": false }),
    )
    .expect("answered");
    assert_eq!(state.navigation_decoupled, vec![ViewId(3)]);
    assert!(state.excluded.is_empty(), "input sync stays");
    assert!(
        crate::sync_hub::mcp::sync_set(
            &setup.core,
            &json!({ "target": 9, "feature": "navigation", "enabled": false }),
        )
        .is_err()
    );
}
