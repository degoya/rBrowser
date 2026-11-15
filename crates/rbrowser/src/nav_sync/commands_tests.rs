use rb_commands::CommandError;
use serde_json::json;

use super::*;
use crate::host::ui::recording::UiCall;
use crate::nav_sync::tests::{started, views_of};
use crate::sync_hub::tests::{attach_all, setup};

#[tokio::test(flavor = "multi_thread")]
async fn toggle_view_navigation_decouples_the_active_view_and_back() {
    let setup = started(&[]);
    let core = &setup.core;
    assert!(matches!(
        run_command(core, TOGGLE_VIEW_NAVIGATION, &Value::Null),
        Some(Err(CommandError::Failed(_)))
    ));
    core.host().set_active_view(ViewId(2)).expect("a view");
    assert_eq!(
        run_command(core, TOGGLE_VIEW_NAVIGATION, &Value::Null),
        Some(Ok(json!({ "view": 2, "syncNavigation": false })))
    );
    assert_eq!(
        run_command(
            core,
            TOGGLE_VIEW_NAVIGATION,
            &json!({ "view": 2, "enabled": true })
        ),
        Some(Ok(json!({ "view": 2, "syncNavigation": true })))
    );
    for bad in [
        json!({ "view": 9 }),
        json!({ "enabled": "yes" }),
        json!({ "view": "x" }),
    ] {
        assert!(
            matches!(
                run_command(core, TOGGLE_VIEW_NAVIGATION, &bad),
                Some(Err(CommandError::InvalidArgs(_)))
            ),
            "{bad}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn toggle_view_navigation_needs_navigation_sync() {
    let setup = setup();
    assert!(matches!(
        run_command(&setup.core, TOGGLE_VIEW_NAVIGATION, &Value::Null),
        Some(Err(CommandError::Failed(_)))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_core_navigation_commands_skip_decoupled_views() {
    let setup = started(&["tablet"]);
    attach_all(&setup).await;
    let core = &setup.core;
    let reached = |views: &[u32], skipped: &[u32]| {
        Some(Ok(
            json!({ "views": views, "skipped": skipped, "failed": [] }),
        ))
    };
    assert_eq!(
        run_command(core, "view.back", &Value::Null),
        reached(&[1, 3], &[2])
    );
    assert_eq!(
        run_command(core, "view.forward", &json!({ "view": 2 })),
        reached(&[2], &[])
    );
    assert_eq!(
        run_command(core, "reload.all", &json!({ "ignoreCache": true })),
        reached(&[1, 2, 3], &[])
    );
    assert_eq!(
        run_command(core, "view.navigate", &json!({ "url": "localhost:5173/a" })),
        reached(&[1, 3], &[2])
    );
    assert_eq!(
        run_command(
            core,
            "view.navigate",
            &json!({ "url": "http://localhost:5173/b", "view": 2 })
        ),
        reached(&[2], &[])
    );
    let backs = views_of(&setup, |call| match call {
        UiCall::Back(view) => Some(*view),
        _ => None,
    });
    assert_eq!(backs, vec![ViewId(1), ViewId(3)]);
    let forwards = views_of(&setup, |call| match call {
        UiCall::Forward(view) => Some(*view),
        _ => None,
    });
    assert_eq!(forwards, vec![ViewId(2)], "its own address field");
    let reloads = views_of(&setup, |call| match call {
        UiCall::Reload(view, true) => Some(*view),
        _ => None,
    });
    assert_eq!(reloads, vec![ViewId(1), ViewId(2), ViewId(3)]);
    let to_a = views_of(&setup, |call| match call {
        UiCall::Navigate(view, url) if url == "http://localhost:5173/a" => Some(*view),
        _ => None,
    });
    assert_eq!(to_a, vec![ViewId(1), ViewId(3)]);
    let to_b = views_of(&setup, |call| match call {
        UiCall::Navigate(view, url) if url == "http://localhost:5173/b" => Some(*view),
        _ => None,
    });
    assert_eq!(to_b, vec![ViewId(2)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn other_commands_and_a_navigate_without_url_are_not_ours() {
    let setup = started(&[]);
    let core = &setup.core;
    assert!(run_command(core, "view.navigate", &Value::Null).is_none());
    assert!(run_command(core, "sync.toggle", &Value::Null).is_none());
    assert!(matches!(
        run_command(core, "view.back", &json!({ "view": 9 })),
        Some(Err(CommandError::InvalidArgs(_)))
    ));
    assert!(matches!(
        run_command(
            core,
            "view.navigate",
            &json!({ "url": "javascript:alert(1)" })
        ),
        Some(Err(CommandError::InvalidArgs(_)))
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_target_reaches_that_view_or_the_coupled_ones() {
    // MCP's `navigate`, `back`, `forward` and `reload` hand the target as `target`.
    let setup = started(&["tablet"]);
    attach_all(&setup).await;
    let core = &setup.core;
    assert_eq!(
        run_command(core, "view.back", &json!({ "target": 2 })),
        Some(Ok(json!({ "views": [2], "skipped": [], "failed": [] })))
    );
    assert_eq!(
        run_command(core, "reload.all", &json!({ "target": "all" })),
        Some(Ok(
            json!({ "views": [1, 2, 3], "skipped": [], "failed": [] })
        ))
    );
    assert!(matches!(
        run_command(core, "view.forward", &json!({ "target": "two" })),
        Some(Err(CommandError::InvalidArgs(_)))
    ));
    let backs = views_of(&setup, |call| match call {
        UiCall::Back(view) => Some(*view),
        _ => None,
    });
    assert_eq!(backs, vec![ViewId(2)]);
    let reloads = views_of(&setup, |call| match call {
        UiCall::Reload(view, _) => Some(*view),
        _ => None,
    });
    assert_eq!(reloads, vec![ViewId(1), ViewId(2), ViewId(3)]);
}
