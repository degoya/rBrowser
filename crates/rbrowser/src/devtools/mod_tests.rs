use rb_engine::ViewHost;

use super::session::tests::{attach_views, docks, setup};
use super::*;
use crate::host::ui::DockSide;
use crate::host::ui::recording::UiCall;

#[tokio::test]
async fn only_devtools_commands_are_answered_here() {
    let setup = setup();
    assert!(run_command(&setup.core, "emulation.set", &json!({})).is_none());
    assert!(run_command(&setup.core, "devtools.toggle", &json!({})).is_some());
}

#[tokio::test]
async fn devtools_open_takes_one_view_and_a_mode() {
    let setup = setup();
    attach_views(&setup).await;
    let all =
        run_command(&setup.core, "devtools.open", &json!({ "target": "all" })).expect("answered");
    assert!(matches!(all, Err(CommandError::InvalidArgs(_))));
    let floating = run_command(
        &setup.core,
        "devtools.open",
        &json!({ "target": 2, "mode": "floating" }),
    )
    .expect("answered");
    assert!(matches!(floating, Err(CommandError::InvalidArgs(_))));
    let window = run_command(
        &setup.core,
        "devtools.open",
        &json!({ "viewId": 2, "mode": "window" }),
    )
    .expect("answered")
    .expect("opens");
    assert_eq!(window, json!({ "open": true, "view": 2, "mode": "window" }));
    assert!(setup.ui.calls().contains(&UiCall::Devtools(ViewId(2))));
}

#[tokio::test]
async fn the_view_host_opens_devtools_through_this_module() {
    let setup = setup();
    attach_views(&setup).await;
    setup
        .core
        .host()
        .open_devtools(ViewId(3), DevtoolsMode::Window)
        .expect("opens");
    assert!(setup.ui.calls().contains(&UiCall::Devtools(ViewId(3))));
    assert_eq!(window_bounds(&setup.core), None, "no window was closed yet");
}

#[tokio::test]
async fn the_dock_cycle_changes_the_setting_keeps_it_and_moves_open_devtools() {
    let setup = setup();
    attach_views(&setup).await;
    let path = setup.core.user_settings_path().expect("a path").to_owned();
    std::fs::write(
        &path,
        r#"{ "theme": "dark", "devtools": { "followActiveView": false } }"#,
    )
    .expect("writes");
    run_command(&setup.core, "devtools.toggle", &json!({}))
        .expect("answered")
        .expect("opens");
    let cycled = run_command(&setup.core, "devtools.dock_cycle", &json!({}))
        .expect("answered")
        .expect("cycles");
    assert_eq!(cycled, json!({ "mode": "docked-right" }));
    assert_eq!(
        setup.core.user_settings().devtools.mode,
        DevtoolsMode::DockedRight
    );
    let written = rb_config::read_json(&path).expect("reads");
    assert_eq!(written["devtools"]["mode"], "docked-right");
    assert_eq!(written["devtools"]["followActiveView"], false);
    assert_eq!(written["theme"], "dark");
    let dock = docks(&setup.ui).last().cloned().flatten().expect("docked");
    assert_eq!(dock.side, DockSide::Right);
    let window = run_command(&setup.core, "devtools.dock_cycle", &json!({}))
        .expect("answered")
        .expect("cycles");
    assert_eq!(window, json!({ "mode": "window" }));
    assert_eq!(docks(&setup.ui).last(), Some(&None));
}

#[tokio::test]
async fn a_changed_setting_moves_open_devtools() {
    let setup = setup();
    attach_views(&setup).await;
    setup
        .devtools
        .open(ViewId(1), DevtoolsMode::DockedBottom)
        .expect("opens");
    let mut user = setup.core.user_settings();
    user.devtools.mode = DevtoolsMode::Window;
    setup.core.replace_user_settings(user);
    on_settings_changed(&setup.core);
    assert!(setup.ui.calls().contains(&UiCall::Devtools(ViewId(1))));
    assert_eq!(
        setup.devtools.current().map(|open| open.mode),
        Some(DevtoolsMode::Window)
    );
}

#[test]
fn persist_mode_leaves_a_file_that_does_not_read_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    std::fs::write(&path, "{ broken").expect("writes");
    persist_mode(&path, DevtoolsMode::Window);
    assert_eq!(std::fs::read_to_string(&path).expect("reads"), "{ broken");
    let newer = format!("{{ \"version\": {} }}", rb_config::CURRENT_VERSION + 1);
    std::fs::write(&path, &newer).expect("writes");
    persist_mode(&path, DevtoolsMode::Window);
    assert_eq!(
        std::fs::read_to_string(&path).expect("reads"),
        newer,
        "a newer rBrowser's file is kept"
    );
    let fresh = dir.path().join("new").join("settings.json");
    persist_mode(&fresh, DevtoolsMode::DockedRight);
    let written = rb_config::read_json(&fresh).expect("reads");
    assert_eq!(written, json!({ "devtools": { "mode": "docked-right" } }));
}

/// Runs `devtools.inspect_at` as the registry does: on a blocking thread of the runtime.
async fn inspect_at(
    setup: &super::session::tests::Setup,
    args: Value,
) -> Result<Value, CommandError> {
    let core = setup.core.clone();
    tokio::task::spawn_blocking(move || {
        run_command(&core, "devtools.inspect_at", &args).expect("this module's command")
    })
    .await
    .expect("joined")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inspect_at_opens_devtools_and_hands_the_node_under_the_point_to_the_frontend() {
    let setup = setup();
    attach_views(&setup).await;
    setup.fake.answer_with(
        "DOM.getNodeForLocation",
        json!({ "backendNodeId": 42, "frameId": "F1" }),
    );
    let answer = inspect_at(&setup, json!({ "view": 2, "x": 30, "y": 40, "dip": true }))
        .await
        .expect("inspects");
    assert_eq!(answer["view"], 2);
    assert_eq!(answer["backendNodeId"], 42);
    assert_eq!(answer["selected"], true);
    assert_eq!(answer["mode"], json!(DevtoolsMode::default()));
    assert_eq!(
        setup.devtools.current().map(|open| open.view),
        Some(ViewId(2)),
        "DevTools opened for the clicked view"
    );
    let scale = setup.core.host().view(ViewId(2)).expect("a view").scale;
    let sent = &setup.fake.sent("DOM.getNodeForLocation")[0];
    assert_eq!(
        sent["sessionId"],
        setup
            .core
            .host()
            .capture_target(ViewId(2))
            .expect("attached")
            .session
    );
    assert_eq!(sent["params"]["x"], json!((30.0 / scale).round() as i64));
    assert_eq!(sent["params"]["y"], json!((40.0 / scale).round() as i64));
    assert_eq!(setup.devtools.pending_inspect(ViewId(2)), Some(42));
    // Closing DevTools forgets the request.
    setup.devtools.close().expect("closes");
    assert_eq!(setup.devtools.pending_inspect(ViewId(2)), None);
}

/// MCP's point is in CSS px of the page: sent as given, whatever the cell's scale.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inspect_at_without_dip_takes_page_px() {
    let setup = setup();
    attach_views(&setup).await;
    setup.fake.answer_with(
        "DOM.getNodeForLocation",
        json!({ "backendNodeId": 5, "frameId": "F1" }),
    );
    inspect_at(&setup, json!({ "view": 2, "x": 400, "y": 300 }))
        .await
        .expect("inspects");
    let sent = &setup.fake.sent("DOM.getNodeForLocation")[0];
    assert_eq!(sent["params"]["x"], 400);
    assert_eq!(sent["params"]["y"], 300);
    let bad = inspect_at(&setup, json!({ "view": 2, "x": 1, "y": 1, "dip": "yes" })).await;
    assert!(matches!(bad, Err(CommandError::InvalidArgs(_))), "{bad:?}");
}

/// The node is resolved before DevTools open: no element, no DevTools.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inspect_at_opens_nothing_when_no_node_is_found() {
    let setup = setup();
    attach_views(&setup).await;
    setup.fake.fail("DOM.getNodeForLocation", true);
    let failed = inspect_at(&setup, json!({ "view": 2, "x": 1, "y": 1 })).await;
    assert!(matches!(failed, Err(CommandError::Failed(_))), "{failed:?}");
    assert!(setup.devtools.current().is_none(), "DevTools stayed closed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inspect_at_refuses_half_a_point_and_an_unattached_view() {
    let setup = setup();
    attach_views(&setup).await;
    let half = inspect_at(&setup, json!({ "view": 1, "x": 3 })).await;
    assert!(
        matches!(half, Err(CommandError::InvalidArgs(_))),
        "{half:?}"
    );
    let negative = inspect_at(&setup, json!({ "view": 1, "x": -1, "y": 2 })).await;
    assert!(
        matches!(negative, Err(CommandError::InvalidArgs(_))),
        "{negative:?}"
    );
    let all = inspect_at(&setup, json!({ "target": "all", "x": 1, "y": 2 })).await;
    assert!(matches!(all, Err(CommandError::InvalidArgs(_))), "{all:?}");
    let unknown = inspect_at(&setup, json!({ "view": 99, "x": 1, "y": 2 })).await;
    assert!(
        matches!(unknown, Err(CommandError::Failed(_))),
        "{unknown:?}"
    );
    assert!(setup.devtools.current().is_none(), "nothing opened");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inspect_at_in_a_window_opens_it_but_selects_nothing() {
    let setup = setup();
    attach_views(&setup).await;
    let mut user = setup.core.user_settings();
    user.devtools.mode = DevtoolsMode::Window;
    setup.core.replace_user_settings(user);
    setup.fake.answer_with(
        "DOM.getNodeForLocation",
        json!({ "backendNodeId": 7, "frameId": "F1" }),
    );
    let answer = inspect_at(&setup, json!({ "view": 3, "x": 1, "y": 1 }))
        .await
        .expect("inspects");
    assert_eq!(answer["selected"], false);
    assert_eq!(answer["backendNodeId"], Value::Null);
    assert!(
        setup.fake.sent("DOM.getNodeForLocation").is_empty(),
        "no round trip for a window that takes no selection"
    );
    assert!(setup.ui.calls().contains(&UiCall::Devtools(ViewId(3))));
}
