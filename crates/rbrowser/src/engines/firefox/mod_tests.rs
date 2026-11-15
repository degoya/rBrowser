use std::path::Path;
use std::sync::Arc;

use rb_bidi::launcher::LocateError;
use rb_bidi::testing::FakeFirefox;
use rb_config::EngineKind;
use rb_engine::ViewId;
use rb_engine::external::ExternalStatus;
use rb_sync::ViewRoute;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::engines::tests::until;
use crate::host::nav::marker_url;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

/// A window whose mobile view (3) runs on Firefox, its UI recorded.
pub(crate) fn setup(runtime: Handle) -> (Arc<Core>, Arc<RecordingUi>) {
    setup_with(runtime, |_| {})
}

/// [`setup`] with `edit` applied to the project config.
fn setup_with(
    runtime: Handle,
    edit: impl FnOnce(&mut rb_config::ProjectConfig),
) -> (Arc<Core>, Arc<RecordingUi>) {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, runtime.clone());
    let mut config = default_config();
    config.breakpoints[2].engine = EngineKind::Firefox;
    edit(&mut config);
    host.open_config(config, None).expect("opens");
    for id in 1..=3 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    let core = Core::new(host, ui.clone(), runtime, CoreSetup::default());
    core.shell_ready();
    (core, ui)
}

/// The window started with `fake` as its Firefox.
pub(crate) async fn started_with(fake: &FakeFirefox) -> (Arc<Core>, Arc<RecordingUi>) {
    started_with_config(fake, |_| {}).await
}

/// [`started_with`] with `edit` applied to the project config.
async fn started_with_config(
    fake: &FakeFirefox,
    edit: impl FnOnce(&mut rb_config::ProjectConfig),
) -> (Arc<Core>, Arc<RecordingUi>) {
    *lock(&TEST_LAUNCH) = Some(fake.launcher());
    let (core, ui) = setup_with(Handle::current(), edit);
    crate::sync_hub::on_start(&core);
    crate::engines::on_start(&core);
    until(|| status(&core) == Some(ExternalStatus::Running)).await;
    (core, ui)
}

/// Attaches the CDP session of `view`'s cell, so navigations reach the window.
pub(crate) async fn attach(core: &Core, view: ViewId) {
    let host = core.host();
    host.on_view_bounds(
        view,
        rb_engine::CellRect {
            x: 0,
            y: 0,
            width: 400,
            height: 300,
        },
    );
    host.on_load_end(view, &marker_url(view));
    until(|| host.sync_agents().config(view).is_some()).await;
}

pub(crate) fn status(core: &Core) -> Option<ExternalStatus> {
    crate::engines::for_host(core.host())
        .and_then(|engines| engines.list().first().map(|view| view.status))
}

fn shell(ui: &RecordingUi) -> String {
    ui.calls()
        .iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) => Some(script.clone()),
            _ => None,
        })
        .collect()
}

/// The profile is never in the project tree (a clone could ship `prefs.js`, a root CA or an
/// extension in it): rBrowser's local folder, keyed by the project like Chromium's profile.
#[test]
fn the_profile_lives_in_the_local_folder_keyed_by_the_project() {
    let root = Path::new("/p");
    let local = Path::new("/l");
    let keyed = crate::project::profile_dir(&local.join("firefox"), Some(root));
    assert_eq!(
        profile_dir(None, Some(root), Some(local)),
        Some(keyed.clone())
    );
    assert_eq!(
        profile_dir(Some("  "), Some(root), Some(local)),
        Some(keyed.clone())
    );
    assert!(!keyed.starts_with(root), "{}", keyed.display());
    assert_ne!(
        profile_dir(None, Some(Path::new("/q")), Some(local)),
        Some(keyed),
        "each project its own"
    );
    assert_eq!(
        profile_dir(None, None, Some(local)),
        Some(crate::project::profile_dir(&local.join("firefox"), None))
    );
    assert_eq!(profile_dir(None, Some(root), None), None);
}

#[test]
fn a_set_profile_folder_is_absolute_or_under_the_local_folder() {
    let root = Path::new("/p");
    let local = Path::new("/l");
    assert_eq!(
        profile_dir(Some("ff"), Some(root), Some(local)),
        Some(local.join("ff")),
        "never joined to the project root"
    );
    assert_eq!(profile_dir(Some("../ff"), Some(root), Some(local)), None);
    assert_eq!(profile_dir(Some("ff"), Some(root), None), None);
    let absolute = std::env::temp_dir().join("rb-ff");
    let absolute_text = absolute.to_string_lossy().into_owned();
    assert_eq!(
        profile_dir(Some(&absolute_text), Some(root), Some(local)),
        Some(absolute)
    );
}

#[test]
fn the_reasons_name_the_path_or_the_setting() {
    let configured = reason_of(&LocateError::Configured("C:\\FF\\firefox.exe".into()));
    assert!(
        configured.contains("C:\\FF\\firefox.exe")
            || configured == "engines.firefox.configuredMissing",
        "{configured}"
    );
    let missing = reason_of(&LocateError::NotFound);
    assert!(
        missing.contains("Firefox path") || missing == "engines.firefox.notFound",
        "{missing}"
    );
}

#[test]
fn toggle_takes_enabled_or_flips() {
    let args: ToggleArgs = serde_json::from_value(json!({ "enabled": false })).expect("parses");
    assert_eq!(args.enabled, Some(false));
    assert!(serde_json::from_value::<ToggleArgs>(json!({ "on": true })).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_firefox_marks_the_view_and_links_the_setting_once() {
    let (core, ui) = setup(Handle::current());
    crate::sync_hub::on_start(&core);
    crate::engines::on_start(&core);
    until(|| status(&core) == Some(ExternalStatus::Unavailable)).await;
    let engines = crate::engines::for_host(core.host()).expect("started");
    assert_eq!(
        engines.list()[0].reason.as_deref(),
        Some(reason_of(&LocateError::NotFound).as_str())
    );
    crate::engines::on_config_changed(&core);
    let scripts = shell(&ui);
    assert_eq!(
        scripts.matches("engines.firefox.notFoundNotice").count(),
        1,
        "{scripts}"
    );
    assert!(scripts.contains(PATH_KEY), "the notice links the setting");
    // The other views run on: the sync still routes to them, Chromium shows view 3.
    let sync = crate::sync_hub::for_host(core.host()).expect("sync");
    assert_eq!(
        sync.router().route(ViewId(1)),
        ViewRoute::Engine(EngineKind::Chromium)
    );
    assert_eq!(
        sync.router().route(ViewId(3)),
        ViewRoute::Engine(EngineKind::Chromium)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_found_firefox_runs_the_view_and_takes_its_sync() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with(&fake).await;
    assert_eq!(fake.calls("browsingContext.create").len(), 1);
    let sync = crate::sync_hub::for_host(core.host()).expect("sync");
    until(|| sync.router().route(ViewId(3)) == ViewRoute::Engine(EngineKind::Firefox)).await;
    let click: rb_sync::SyncEvent = serde_json::from_value(json!({
        "type": "click", "button": "left", "detail": 1, "modifiers": 0,
        "target": { "element": { "kind": "css", "path": "#go" }, "offset": { "x": 0.5, "y": 0.5 } }
    }))
    .expect("event");
    let request = rb_sync::ReplayRequest {
        source: ViewId(1),
        target: ViewId(3),
        top: true,
        frame: None,
        event: click.into(),
        block_submit: false,
    };
    use rb_sync::Replay;
    assert_eq!(
        sync.router().dispatch(request).await,
        rb_sync::ReplayOutcome::Done
    );
    assert_eq!(
        fake.calls("input.performActions").len(),
        2,
        "press and release"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_toggle_stops_and_starts_the_firefox_windows() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with(&fake).await;
    let off = run_command(&core, TOGGLE, &json!({ "enabled": false })).expect("ours");
    assert_eq!(off.expect("runs"), json!({ "enabled": false }));
    until(|| fake.calls("browser.close").len() == 1).await;
    until(|| {
        matches!(
            status(&core),
            Some(ExternalStatus::Unavailable | ExternalStatus::Exited)
        )
    })
    .await;
    let on = run_command(&core, TOGGLE, &Value::Null).expect("ours");
    assert_eq!(on.expect("runs"), json!({ "enabled": true }));
    until(|| status(&core) == Some(ExternalStatus::Running)).await;
    assert_eq!(fake.calls("session.new").len(), 2, "a fresh Firefox");
    assert!(run_command(&core, "engine.restart", &Value::Null).is_none());
}

/// CEF runs the start hooks on its UI thread, outside the runtime: Firefox's part needs no
/// runtime context either.
#[test]
fn a_found_firefox_starts_outside_the_runtime() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let fake = runtime.block_on(FakeFirefox::start());
    *lock(&TEST_LAUNCH) = Some(fake.launcher());
    let (core, _ui) = {
        let _inside = runtime.enter();
        setup(runtime.handle().clone())
    };
    crate::sync_hub::on_start(&core);
    crate::engines::on_start(&core);
    on_config_changed(&core);
    runtime.block_on(until(|| status(&core) == Some(ExternalStatus::Running)));
    crate::engines::shutdown(&runtime);
    assert_eq!(
        fake.calls("browser.close").len(),
        1,
        "Firefox quits with the app"
    );
}

/// The app waits `SHUTDOWN_WAIT` for the external windows; Firefox's orderly quit fits in it,
/// so the kill after it is never cut off.
#[test]
fn firefox_quits_within_the_apps_wait() {
    assert!(rb_bidi::firefox::QUIT_BUDGET < crate::engines::SHUTDOWN_WAIT);
}

#[tokio::test(flavor = "multi_thread")]
async fn turning_a_running_firefox_on_changes_nothing() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with(&fake).await;
    let on = run_command(&core, TOGGLE, &json!({ "enabled": true })).expect("ours");
    assert_eq!(on.expect("runs"), json!({ "enabled": true }));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(fake.calls("session.new").len(), 1, "no second Firefox");
    assert!(
        fake.calls("browser.close").is_empty(),
        "the first one runs on"
    );
}

/// Off and on at once: the new Firefox starts on the profile only after the old one quit.
#[tokio::test(flavor = "multi_thread")]
async fn off_and_on_at_once_waits_for_the_old_firefox() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with(&fake).await;
    run_command(&core, TOGGLE, &json!({ "enabled": false }))
        .expect("ours")
        .expect("off");
    run_command(&core, TOGGLE, &json!({ "enabled": true }))
        .expect("ours")
        .expect("on");
    until(|| fake.calls("session.new").len() == 2).await;
    let methods: Vec<String> = fake.commands().into_iter().map(|(m, _)| m).collect();
    let quit = methods
        .iter()
        .position(|m| m == "browser.close")
        .expect("the old Firefox quit");
    let second = methods
        .iter()
        .enumerate()
        .filter(|(_, m)| *m == "session.new")
        .nth(1)
        .map(|(at, _)| at)
        .expect("a second session");
    assert!(quit < second, "{methods:?}");
}

/// `emulation.set` on a Firefox view reaches its window, not only the stand-in.
#[tokio::test(flavor = "multi_thread")]
async fn a_changed_emulation_reaches_the_firefox_window() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with(&fake).await;
    let fields = rb_config::EmulationFields {
        user_agent: Some("UA-X".to_owned()),
        ..Default::default()
    };
    core.host()
        .set_emulation_fields(ViewId(3), &fields)
        .expect("set");
    until(|| {
        fake.calls("emulation.setUserAgentOverride")
            .iter()
            .any(|call| call["userAgent"] == "UA-X")
    })
    .await;
}

/// The project's headers wait for workspace trust in Firefox too, and go when it is revoked.
#[tokio::test(flavor = "multi_thread")]
async fn project_headers_reach_firefox_only_in_a_trusted_workspace() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with_config(&fake, |config| {
        config.breakpoints[2]
            .emulation
            .headers
            .insert("X-Project".to_owned(), "1".to_owned());
    })
    .await;
    let last = || fake.calls("network.setExtraHeaders").pop();
    until(|| last().is_some()).await;
    assert_eq!(last().expect("sent")["headers"], json!([]), "untrusted");
    core.host().update_shell_state(|shell| shell.trusted = true);
    until(|| last().is_some_and(|call| call["headers"] != json!([]))).await;
    core.host()
        .update_shell_state(|shell| shell.trusted = false);
    until(|| last().is_some_and(|call| call["headers"] == json!([]))).await;
}

/// The Firefox agents follow the hub's switches - sync off, a kind off - not only the
/// project's `sync.*`.
#[tokio::test(flavor = "multi_thread")]
async fn the_firefox_agents_follow_the_hubs_switches() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with(&fake).await;
    let firefox = for_host(core.host())
        .and_then(|bridge| bridge.firefox())
        .expect("Firefox runs");
    assert!(firefox.agent_config().active, "a sync source captures");
    let sync = crate::sync_hub::for_host(core.host()).expect("sync");
    sync.set_kind(rb_sync::SyncKind::Click, false);
    until(|| !firefox.agent_config().capture.click).await;
    sync.set_enabled(false);
    until(|| !firefox.agent_config().active).await;
}

/// The profile hides the tabs and toolbars while the views sit in their cells
/// (`engines.external.embed`, RB-080-39); a change rebuilds the engine like a changed path.
#[tokio::test(flavor = "multi_thread")]
async fn the_firefox_chrome_is_hidden_while_the_views_sit_in_their_cells() {
    let (core, _ui) = setup(Handle::current());
    assert!(!launching(&core).hide_chrome, "off by default");
    let mut user = core.user_settings();
    user.engines.external.embed = true;
    core.replace_user_settings(user);
    assert!(launching(&core).hide_chrome);
}

/// The Firefox views in their cells render at one factor per profile (RB-080-40,
/// `layout.css.devPixelsPerPx`): the first measured cell starts Firefox anew with it, a cell a
/// few pixels wider or narrower does not, and with the embedding off the pref is left alone.
#[tokio::test(flavor = "multi_thread")]
async fn a_measured_cell_restarts_firefox_at_the_shared_factor() {
    let fake = FakeFirefox::start().await;
    let (core, _ui) = started_with(&fake).await;
    assert_eq!(launching(&core).render_scale, None, "embedding off");
    let mut user = core.user_settings();
    user.engines.external.embed = true;
    core.replace_user_settings(user);
    crate::engines::on_config_changed(&core);
    until(|| fake.calls("session.new").len() == 2).await;
    until(|| status(&core) == Some(ExternalStatus::Running)).await;
    let engines = crate::engines::for_host(core.host()).expect("engines");
    // The mobile breakpoint is 393 wide; a cell of 315 x 500 px: factor 0.8.
    engines.cell_sized(
        ViewId(3),
        rb_engine::external::CellPixels {
            width: 315,
            height: 500,
        },
    );
    until(|| engines.firefox_render_scale() == Some(0.8)).await;
    assert_eq!(launching(&core).render_scale, Some(0.8));
    until(|| fake.calls("session.new").len() == 3).await;
    until(|| status(&core) == Some(ExternalStatus::Running)).await;
    // The view started anew opens at the cell's height.
    until(|| {
        fake.calls("browsingContext.setViewport")
            .iter()
            .any(|call| call["viewport"] == json!({ "width": 393, "height": 625 }))
    })
    .await;
    // One pixel wider: the same step, no restart; a much narrower cell: a new step.
    engines.cell_sized(
        ViewId(3),
        rb_engine::external::CellPixels {
            width: 316,
            height: 500,
        },
    );
    tokio::time::sleep(crate::engines::embed::cells::SETTLE * 3).await;
    assert_eq!(engines.firefox_render_scale(), Some(0.8));
    assert_eq!(fake.calls("session.new").len(), 3);
    engines.cell_sized(
        ViewId(3),
        rb_engine::external::CellPixels {
            width: 280,
            height: 500,
        },
    );
    until(|| engines.firefox_render_scale() == Some(0.7)).await;
    until(|| fake.calls("session.new").len() == 4).await;
    until(|| status(&core) == Some(ExternalStatus::Running)).await;
    // The last view shown as a window of its own keeps the profile's factor: no restart for
    // a pop-out, none for the pop-in (review finding 3).
    engines.release(ViewId(3));
    tokio::time::sleep(crate::engines::embed::cells::SETTLE * 3).await;
    assert_eq!(engines.firefox_render_scale(), Some(0.7));
    assert_eq!(fake.calls("session.new").len(), 4);
    engines.embed_again(ViewId(3));
    tokio::time::sleep(crate::engines::embed::cells::SETTLE * 3).await;
    assert_eq!(fake.calls("session.new").len(), 4);
}
