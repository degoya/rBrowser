use std::sync::Arc;

use rb_config::EngineKind;
use rb_engine::ViewId;
use rb_engine::external::ExternalStatus;
use rb_sync::ViewRoute;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::fake::FakeSidecar;
use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::engines::tests::until;
use crate::host::nav::marker_url;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

/// The default window with its mobile view (3) on WebKit.
fn setup_on(runtime: Handle) -> Arc<Core> {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, runtime.clone());
    let mut config = default_config();
    config.breakpoints[2].engine = EngineKind::Webkit;
    host.open_config(config, None).expect("opens");
    for id in 1..=3 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    Core::new(host, ui, runtime, CoreSetup::default())
}

fn status(core: &Core) -> Option<(ExternalStatus, Option<String>)> {
    crate::engines::for_host(core.host())?
        .list()
        .into_iter()
        .find(|view| view.view == ViewId(3))
        .map(|view| (view.status, view.reason))
}

fn running(core: &Core) -> bool {
    status(core).is_some_and(|(status, _)| status == ExternalStatus::Running)
}

/// The window started with WebKit plugged in over `fake`.
fn start(core: &Core, fake: &Arc<FakeSidecar>) {
    crate::sync_hub::on_start(core);
    crate::engines::start_with(core, Some(Arc::new(fake.clone())));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_webkit_view_runs_in_its_window_and_the_sync_routes_to_it() {
    let core = setup_on(Handle::current());
    let fake = FakeSidecar::new();
    start(&core, &fake);
    until(|| running(&core)).await;
    let create = &fake.calls_of(rpc::CREATE)[0];
    assert_eq!(create["view"], 3);
    let sync = crate::sync_hub::for_host(core.host()).expect("sync runs");
    until(|| sync.router().route(ViewId(3)) == ViewRoute::Engine(EngineKind::Webkit)).await;
    let engines = crate::engines::for_host(core.host()).expect("started");
    let view = &engines.list()[0];
    assert_eq!(view.engine, EngineKind::Webkit);
    // WebKit has no option for these; the default emulation asks for none, so nothing shows.
    assert!(view.unsupported.is_empty(), "{:?}", view.unsupported);
    // A second start changes nothing.
    crate::engines::start_with(&core, Some(Arc::new(fake.clone())));
    assert_eq!(fake.launches(), 1);
}

/// The engine is in place before the first view starts: the view never shows a stand-in's
/// "not available" on its way to running (RB-040-04 review, finding 4).
#[tokio::test(flavor = "multi_thread")]
async fn the_view_starts_on_webkit_never_on_a_stand_in() {
    let core = setup_on(Handle::current());
    let fake = FakeSidecar::new();
    start(&core, &fake);
    let mut seen = Vec::new();
    until(|| {
        let now = status(&core);
        let done = now
            .as_ref()
            .is_some_and(|(status, _)| *status == ExternalStatus::Running);
        seen.extend(now);
        done
    })
    .await;
    assert!(
        seen.iter()
            .all(|(status, _)| *status != ExternalStatus::Unavailable),
        "{seen:?}"
    );
    assert_eq!(fake.calls_of(rpc::CREATE).len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_toggle_switches_webkit_off_and_on() {
    let core = setup_on(Handle::current());
    let fake = FakeSidecar::new();
    start(&core, &fake);
    until(|| running(&core)).await;
    let off = run_command(&core, TOGGLE, &Value::Null).expect("the toggle");
    assert_eq!(off.expect("answers"), json!({ "enabled": false }));
    until(|| {
        status(&core).is_some_and(|(status, reason)| {
            status == ExternalStatus::Unavailable
                && reason == Some(rb_i18n::t("engines.webkit.disabled"))
        })
    })
    .await;
    let on = run_command(&core, TOGGLE, &Value::Null).expect("the toggle");
    assert_eq!(on.expect("answers"), json!({ "enabled": true }));
    until(|| running(&core)).await;
    assert!(run_command(&core, "engine.restart", &Value::Null).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_build_the_view_says_so_and_starts_once_it_is_installed() {
    let core = setup_on(Handle::current());
    let fake = FakeSidecar::new();
    fake.fail_next(
        rpc::LAUNCH,
        rpc::NOT_INSTALLED,
        "WebKit 2388 is not installed",
    );
    start(&core, &fake);
    let missing = rb_i18n::t("engines.webkit.notInstalled");
    until(|| {
        status(&core).is_some_and(|(status, reason)| {
            status != ExternalStatus::Running && reason.as_deref() == Some(missing.as_str())
        })
    })
    .await;
    installed();
    until(|| running(&core)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_sidecar_webkit_is_unavailable_and_says_why() {
    let core = setup_on(Handle::current());
    crate::engines::start_with(&core, None);
    let reason = rb_i18n::t("engines.webkit.noSidecar");
    until(|| status(&core) == Some((ExternalStatus::Unavailable, Some(reason.clone())))).await;
    let error = run_command(&core, TOGGLE, &Value::Null)
        .expect("the toggle")
        .expect_err("no engine");
    assert!(error.to_string().contains(&reason), "{error}");
}

/// CEF runs the start hooks on its UI thread, outside the runtime.
#[test]
fn the_start_runs_outside_the_runtime() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let core = {
        let _inside = runtime.enter();
        setup_on(runtime.handle().clone())
    };
    let fake = FakeSidecar::new();
    crate::engines::start_with(&core, Some(Arc::new(fake.clone())));
    on_config_changed(&core);
    runtime.block_on(until(|| running(&core)));
    crate::engines::shutdown(&runtime);
    assert_eq!(fake.calls_of(rpc::SHUTDOWN).len(), 1);
}

/// RB-090-33 review: WebKit's agents take the scroll mode in effect - the one `sync_set` set at
/// runtime - not only the project's `sync.scrollMode`.
#[tokio::test]
async fn the_agents_take_the_scroll_mode_in_effect() {
    let core = setup_on(Handle::current());
    assert_eq!(sync_of(&core).scroll_mode, rb_config::ScrollMode::Anchor);
    crate::scroll_sync::set_mode(core.host(), Some(rb_config::ScrollMode::Wheel));
    assert_eq!(sync_of(&core).scroll_mode, rb_config::ScrollMode::Wheel);
}
