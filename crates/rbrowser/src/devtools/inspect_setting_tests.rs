//! `devtools.openOnInspect` and the `open` argument of `devtools.inspect_at` (RB-080-45).

use rb_config::DevtoolsMode;
use rb_engine::ViewId;
use rb_ipc::NotificationLevel;
use serde_json::{Value, json};

use super::inspect::{NOT_OPEN_KEY, OPEN_SETTING};
use super::run_command;
use super::session::tests::{Setup, attach_views, setup, until};

async fn inspect_at(setup: &Setup, args: Value) -> Value {
    let core = setup.core.clone();
    tokio::task::spawn_blocking(move || {
        run_command(&core, "devtools.inspect_at", &args).expect("this module's command")
    })
    .await
    .expect("joined")
    .expect("answers")
}

async fn setting_off() -> Setup {
    let setup = setup();
    attach_views(&setup).await;
    let mut user = setup.core.user_settings();
    user.devtools.open_on_inspect = false;
    setup.core.replace_user_settings(user);
    setup.fake.answer_with(
        "DOM.getNodeForLocation",
        json!({ "backendNodeId": 9, "frameId": "F1" }),
    );
    setup
}

/// Off: closed DevTools stay closed, nothing is resolved, and the status bar says so with the
/// setting linked - a notice, not a failure.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn off_leaves_closed_devtools_closed_and_links_the_setting() {
    let setup = setting_off().await;
    let answer = inspect_at(&setup, json!({ "view": 2, "x": 1, "y": 1 })).await;
    assert_eq!(answer["open"], false);
    assert_eq!(answer["selected"], false);
    assert_eq!(answer["reason"], OPEN_SETTING);
    assert!(setup.devtools.current().is_none(), "DevTools stayed closed");
    assert!(
        setup.fake.sent("DOM.getNodeForLocation").is_empty(),
        "nothing resolved for DevTools that do not open"
    );
    let messages = setup.core.status_log().items();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert_eq!(messages[0].message_key, NOT_OPEN_KEY);
    assert_eq!(messages[0].level, NotificationLevel::Warning);
    assert_eq!(messages[0].settings_key.as_deref(), Some(OPEN_SETTING));
}

/// Off: DevTools open for another view are not moved either.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn off_does_not_move_devtools_from_another_view() {
    let setup = setting_off().await;
    setup
        .devtools
        .open(ViewId(3), DevtoolsMode::default())
        .expect("opens");
    let answer = inspect_at(&setup, json!({ "view": 2, "x": 1, "y": 1 })).await;
    assert_eq!(answer["open"], false);
    assert_eq!(
        setup.devtools.current().map(|open| open.view),
        Some(ViewId(3)),
        "DevTools stay with their view"
    );
    assert_eq!(setup.devtools.pending_inspect(ViewId(2)), None);
    assert_eq!(setup.core.status_log().items().len(), 1);
}

/// Off: DevTools open for the view take the element.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn off_selects_in_devtools_open_for_the_view() {
    let setup = setting_off().await;
    setup
        .devtools
        .open(ViewId(2), DevtoolsMode::default())
        .expect("opens");
    let answer = inspect_at(&setup, json!({ "view": 2, "x": 1, "y": 1 })).await;
    assert_eq!(answer["open"], true);
    assert_eq!(answer["selected"], true);
    assert_eq!(setup.devtools.pending_inspect(ViewId(2)), Some(9));
    assert!(setup.core.status_log().items().is_empty());
}

/// The open state is read again after the resolution: DevTools that closed while the node was
/// resolved are not opened again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn off_does_not_reopen_devtools_that_closed_during_the_resolution() {
    let setup = setting_off().await;
    setup
        .devtools
        .open(ViewId(2), DevtoolsMode::default())
        .expect("opens");
    setup.fake.hold("DOM.getNodeForLocation", None);
    let core = setup.core.clone();
    let running = tokio::task::spawn_blocking(move || {
        run_command(
            &core,
            "devtools.inspect_at",
            &json!({ "view": 2, "x": 1, "y": 1 }),
        )
        .expect("this module's command")
    });
    until(|| !setup.fake.sent("DOM.getNodeForLocation").is_empty()).await;
    setup.devtools.close().expect("closes");
    setup.fake.release();
    let answer = running.await.expect("joined").expect("answers");
    assert_eq!(answer["open"], false, "{answer}");
    assert!(setup.devtools.current().is_none(), "not opened again");
    assert_eq!(setup.devtools.pending_inspect(ViewId(2)), None);
    assert_eq!(setup.core.status_log().items()[0].message_key, NOT_OPEN_KEY);
}

/// `open: true` (MCP) opens whatever the setting says; `open: false` holds back with it on.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_open_argument_overrides_the_setting() {
    let setup = setting_off().await;
    let answer = inspect_at(&setup, json!({ "view": 2, "x": 1, "y": 1, "open": true })).await;
    assert_eq!(answer["open"], true);
    assert_eq!(answer["selected"], true);
    assert_eq!(
        setup.devtools.current().map(|open| open.view),
        Some(ViewId(2))
    );
    assert!(setup.core.status_log().items().is_empty());

    let on = setup_on().await;
    let held = inspect_at(&on, json!({ "view": 2, "x": 1, "y": 1, "open": false })).await;
    assert_eq!(held["open"], false);
    assert!(on.devtools.current().is_none());
    let core = on.core.clone();
    let bad = tokio::task::spawn_blocking(move || {
        run_command(
            &core,
            "devtools.inspect_at",
            &json!({ "view": 2, "open": "yes" }),
        )
        .expect("this module's command")
    })
    .await
    .expect("joined");
    assert!(
        matches!(bad, Err(rb_commands::CommandError::InvalidArgs(_))),
        "{bad:?}"
    );
}

async fn setup_on() -> Setup {
    let setup = setup();
    attach_views(&setup).await;
    setup
}

/// State and mode are read again after the resolution in the opening path too: DevTools that
/// closed while the node was resolved open in the mode the settings give now, not in the one
/// read before the wait.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_mode_is_read_again_after_the_resolution() {
    let setup = setup_on().await;
    setup.fake.answer_with(
        "DOM.getNodeForLocation",
        json!({ "backendNodeId": 9, "frameId": "F1" }),
    );
    setup
        .devtools
        .open(ViewId(2), DevtoolsMode::DockedBottom)
        .expect("opens");
    setup.fake.hold("DOM.getNodeForLocation", None);
    let core = setup.core.clone();
    let running = tokio::task::spawn_blocking(move || {
        run_command(
            &core,
            "devtools.inspect_at",
            &json!({ "view": 2, "x": 1, "y": 1 }),
        )
        .expect("this module's command")
    });
    until(|| !setup.fake.sent("DOM.getNodeForLocation").is_empty()).await;
    setup.devtools.close().expect("closes");
    let mut user = setup.core.user_settings();
    user.devtools.mode = DevtoolsMode::Window;
    setup.core.replace_user_settings(user);
    setup.fake.release();
    let answer = running.await.expect("joined").expect("answers");
    assert_eq!(answer["mode"], json!(DevtoolsMode::Window), "{answer}");
    assert_eq!(
        setup.devtools.current().map(|open| open.mode),
        Some(DevtoolsMode::Window)
    );
    assert_eq!(answer["selected"], false, "a window takes no selection");
}
