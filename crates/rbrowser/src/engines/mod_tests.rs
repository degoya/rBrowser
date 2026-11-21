use std::sync::Arc;

use rb_config::{EngineKind, ExternalWindowSettings};
use rb_engine::ViewId;
use rb_engine::external::{ExternalStatus, Placement};
use rb_engine::test_support::FakeEngine;
use rb_sync::ViewRoute;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::nav::marker_url;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

/// Lets the spawned tasks run until `done` holds (or fails the test after about ten seconds - a
/// give-up deadline, generous for a loaded machine running every test at once).
pub(crate) async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..2000 {
        if done() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

/// A window whose third view (mobile) runs on Firefox: views 1 and 2 on Chromium, 3 external.
pub(crate) fn setup_with_firefox() -> Arc<Core> {
    setup_on(EngineKind::Firefox, Handle::current())
}

/// The default window: desktop, tablet and mobile, the mobile one on `mobile`.
fn setup_with(mobile: EngineKind) -> Arc<Core> {
    setup_on(mobile, Handle::current())
}

fn setup_on(mobile: EngineKind, runtime: Handle) -> Arc<Core> {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, runtime.clone());
    let mut config = default_config();
    config.breakpoints[2].engine = mobile;
    host.open_config(config, None).expect("opens");
    for id in 1..=3 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    Core::new(host, ui, runtime, CoreSetup::default())
}

fn externals(core: &Core) -> Vec<(ViewId, ExternalStatus)> {
    core.host()
        .shell_state()
        .externals
        .iter()
        .map(|view| (view.view, view.status))
        .collect()
}

fn run(core: &Core, id: &str, args: Value) -> Result<Value, rb_commands::CommandError> {
    run_command(core, id, &args).expect("an engines command")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_firefox_view_shows_not_available_yet() {
    let core = setup_with_firefox();
    on_start(&core);
    let first = for_host(core.host()).expect("started");
    on_start(&core);
    assert!(Arc::ptr_eq(
        &first,
        &for_host(core.host()).expect("still there")
    ));
    until(|| externals(&core) == [(ViewId(3), ExternalStatus::Unavailable)]).await;
    let view = &core.host().shell_state().externals[0];
    assert_eq!(view.engine, EngineKind::Firefox);
    assert_eq!(view.breakpoint, "mobile");
    // Unit tests never find the machine's Firefox (RB-040-02): the view says it is missing.
    let missing = firefox::reason_of(&rb_bidi::launcher::LocateError::NotFound);
    assert_eq!(view.reason.as_deref(), Some(missing.as_str()));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_tiles_can_be_switched_off() {
    let core = setup_with_firefox();
    on_start(&core);
    let engines = for_host(core.host()).expect("started");
    until(|| !externals(&core).is_empty()).await;
    engines.apply_settings(&ExternalWindowSettings {
        show_tiles: false,
        ..ExternalWindowSettings::default()
    });
    assert!(externals(&core).is_empty());
    // The view is still known to views_list.
    assert_eq!(engines.list().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_without_an_external_view_says_so() {
    let core = setup_with(EngineKind::Chromium);
    on_start(&core);
    let error = run(&core, RESTART, json!({ "engine": "firefox" })).expect_err("none");
    assert!(
        error.to_string().contains("no Firefox view is open"),
        "{error}"
    );
    let error = run(&core, RESTART, Value::Null).expect_err("none");
    assert!(error.to_string().contains("view is open"), "{error}");
    let error = run(&core, RESTART, json!({ "engine": "chromium" })).expect_err("grid");
    assert!(error.to_string().contains("Chromium"), "{error}");
    let error = run(&core, RESTART, json!({ "colour": "red" })).expect_err("bad args");
    assert!(matches!(error, rb_commands::CommandError::InvalidArgs(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_of_an_unavailable_engine_names_it() {
    let core = setup_with_firefox();
    on_start(&core);
    until(|| !externals(&core).is_empty()).await;
    let error = run(&core, RESTART, json!({ "engine": "firefox" })).expect_err("unavailable");
    let missing = firefox::reason_of(&rb_bidi::launcher::LocateError::NotFound);
    assert!(error.to_string().contains(&missing), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plugged_in_engine_runs_the_view_and_the_sync_routes_to_it() {
    let core = setup_with_firefox();
    crate::sync_hub::on_start(&core);
    on_start(&core);
    let engines = for_host(core.host()).expect("started");
    until(|| externals(&core) == [(ViewId(3), ExternalStatus::Unavailable)]).await;
    let firefox = FakeEngine::new(EngineKind::Firefox);
    engines.windows().set_engine(firefox.clone());
    let answer = run(&core, RESTART, json!({ "view": 3 })).expect("restarts");
    assert_eq!(answer, json!({ "restarted": [3] }));
    until(|| externals(&core) == [(ViewId(3), ExternalStatus::Running)]).await;
    let sync = crate::sync_hub::for_host(core.host()).expect("sync runs");
    assert_eq!(
        sync.router().route(ViewId(3)),
        ViewRoute::Engine(EngineKind::Firefox)
    );
    assert_eq!(
        sync.router().route(ViewId(1)),
        ViewRoute::Engine(EngineKind::Chromium)
    );
    // The window ends: the badge at once, and the sync leaves the view alone until it is back.
    firefox.view(ViewId(3)).expect("opened").crash();
    until(|| sync.router().route(ViewId(3)) == ViewRoute::Stopped).await;
    until(|| externals(&core) == [(ViewId(3), ExternalStatus::Running)]).await;
    assert_eq!(firefox.views().len(), 2);
    // Raise and arrange answer for the running window.
    assert_eq!(
        run(&core, RAISE, Value::Null).expect("raises"),
        json!({ "raised": 1 })
    );
    assert!(run(&core, ARRANGE, Value::Null).expect("arranges")["moved"].is_array());
}

#[test]
fn a_broken_placement_falls_back_to_beside() {
    let settings = |placement: &str| ExternalWindowSettings {
        placement: placement.into(),
        ..ExternalWindowSettings::default()
    };
    assert_eq!(placement_of(&settings("monitor:2")), Placement::Monitor(2));
    assert_eq!(placement_of(&settings("remember")), Placement::Remember);
    assert_eq!(placement_of(&settings("left")), Placement::Beside);
}

#[tokio::test(flavor = "multi_thread")]
async fn other_commands_are_not_the_engines() {
    let core = setup_with_firefox();
    assert!(run_command(&core, "sync.toggle", &Value::Null).is_none());
    // Before the start the engines' commands say so.
    let error = run(&core, ARRANGE, Value::Null).expect_err("not started");
    assert!(error.to_string().contains("not started"));
}

/// CEF runs the start hooks on its UI thread, outside the runtime: neither the start nor the
/// shutdown may need a runtime context.
#[test]
fn start_and_shutdown_run_outside_the_runtime() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let core = {
        let _inside = runtime.enter();
        setup_on(EngineKind::Firefox, runtime.handle().clone())
    };
    on_start(&core);
    on_config_changed(&core);
    runtime.block_on(until(|| {
        externals(&core) == [(ViewId(3), ExternalStatus::Unavailable)]
    }));
    shutdown(&runtime);
    assert!(for_host(core.host()).is_none());
}

/// RB-096-09 (`diag34`): `engines.android.enabled` switched on in Settings is on at once -
/// `view.add_android` said `android.disabled` until a restart, because a change of the user
/// settings never reached the engines.
#[tokio::test(flavor = "multi_thread")]
async fn android_switched_on_in_the_user_settings_is_on_without_a_restart() {
    let core = setup_with(EngineKind::Chromium);
    start_with(&core, None);
    let android = android::for_host(core.host()).expect("started");
    assert!(!android.is_enabled(), "off by default");
    let mut user = core.user_settings();
    user.engines.android.enabled = true;
    core.replace_user_settings(user);
    assert!(android.is_enabled());
    let mut user = core.user_settings();
    user.engines.android.enabled = false;
    core.replace_user_settings(user);
    assert!(!android.is_enabled());
}

/// RB-096-09: `engines.external` follows the user settings too - the tiles go at once.
#[tokio::test(flavor = "multi_thread")]
async fn the_external_settings_follow_the_user_settings_at_once() {
    let core = setup_with_firefox();
    start_with(&core, None);
    until(|| externals(&core) == [(ViewId(3), ExternalStatus::Unavailable)]).await;
    let mut user = core.user_settings();
    user.engines.external.show_tiles = false;
    core.replace_user_settings(user);
    until(|| externals(&core).is_empty()).await;
}
