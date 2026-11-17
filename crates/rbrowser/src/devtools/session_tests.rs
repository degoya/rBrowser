use std::time::Duration;

use futures_util::SinkExt;
use rb_engine::CellRect;
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::devtools::{hidden, screencast};
use crate::host::nav::marker_url;
use crate::host::state::default_config;
use crate::host::ui::DockSide;
use crate::host::ui::recording::{RecordingUi, UiCall};

pub(crate) struct Setup {
    pub(crate) fake: Arc<FakeBrowser>,
    pub(crate) ui: Arc<RecordingUi>,
    pub(crate) core: Arc<Core>,
    pub(crate) devtools: Arc<Devtools>,
    pub(crate) _dir: tempfile::TempDir,
}

pub(crate) fn setup_with(user: rb_config::UserSettings) -> Setup {
    let dir = tempfile::tempdir().expect("temp dir");
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    for id in 1..=3 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    fake.add_target("DOCK", DOCK_MARKER_URL);
    let core = Core::new(
        host,
        ui.clone(),
        Handle::current(),
        CoreSetup {
            user,
            user_path: Some(dir.path().join("settings.json")),
            ..CoreSetup::default()
        },
    );
    let devtools = super::super::start(
        &core,
        Handle::current(),
        Some(dir.path().join("devtools.json")),
    );
    Setup {
        fake,
        ui,
        core,
        devtools,
        _dir: dir,
    }
}

pub(crate) fn setup() -> Setup {
    setup_with(rb_config::UserSettings::default())
}

pub(crate) async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..400 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

/// Lays out and attaches views 1 to 3 (sessions S1 to S3).
pub(crate) async fn attach_views(setup: &Setup) {
    let host = setup.core.host();
    for id in 1..=3 {
        let view = ViewId(id);
        host.on_view_bounds(
            view,
            CellRect {
                x: 0,
                y: 0,
                width: 400,
                height: 300,
            },
        );
        host.on_load_end(view, &marker_url(view));
        until(|| host.target_id(view).is_some()).await;
    }
}

pub(crate) fn docks(ui: &RecordingUi) -> Vec<Option<DevtoolsDock>> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Dock(dock) => Some(dock),
            _ => None,
        })
        .collect()
}

/// Opens docked DevTools for view 1 and lets the dock prepare.
pub(crate) async fn open_prepared(setup: &Setup) -> DevtoolsDock {
    setup
        .devtools
        .open(ViewId(1), DevtoolsMode::DockedBottom)
        .expect("opens");
    setup.devtools.on_dock_loaded(DOCK_MARKER_URL);
    until(|| docks(&setup.ui).len() == 2).await;
    docks(&setup.ui)[1].clone().expect("the frontend")
}

fn metrics_for(setup: &Setup, session: &str) -> usize {
    setup
        .fake
        .sent("Emulation.setDeviceMetricsOverride")
        .iter()
        .filter(|message| message["sessionId"] == session)
        .count()
}

#[tokio::test]
async fn docking_creates_the_dock_at_its_marker_then_prepares_it_and_loads_the_frontend() {
    let setup = setup();
    attach_views(&setup).await;
    setup
        .devtools
        .open(ViewId(1), DevtoolsMode::DockedBottom)
        .expect("opens");
    let first = docks(&setup.ui)[0].clone().expect("a dock");
    assert_eq!(first.frontend_url, DOCK_MARKER_URL);
    assert_eq!(
        (first.view, first.side, first.size),
        (ViewId(1), DockSide::Bottom, 320)
    );
    assert_eq!(
        setup.core.host().shell_state().devtools,
        Some(DevtoolsState {
            view: ViewId(1),
            mode: DevtoolsMode::DockedBottom
        })
    );
    // Another URL loading in the dock changes nothing.
    setup.devtools.on_dock_loaded("about:blank");
    setup.devtools.on_dock_loaded(DOCK_MARKER_URL);
    until(|| docks(&setup.ui).len() == 2).await;
    let frontend = docks(&setup.ui)[1].clone().expect("the frontend");
    assert!(
        frontend
            .frontend_url
            .starts_with("devtools://devtools/bundled/inspector.html?ws=127.0.0.1:"),
        "a frontend URL"
    );
    assert!(hidden::is_hidden("DOCK", "https://anything.test/"));
    // The grid views get the sync agent (RB-020-01); the dock gets only the screencast switch,
    // after `Page.enable` (without it Blink never injects the script, RB-080-21).
    let scripts = dock_scripts(&setup);
    assert_eq!(scripts.len(), 1, "{scripts:#?}");
    assert_eq!(
        scripts[0]["params"]["source"],
        screencast::preference_script(false)
    );
    let enabled: Vec<_> = setup
        .fake
        .sent("Page.enable")
        .into_iter()
        .filter(|message| message["sessionId"] == "S4")
        .collect();
    assert_eq!(enabled.len(), 1, "{enabled:#?}");
    assert!(
        enabled[0]["id"].as_u64() < scripts[0]["id"].as_u64(),
        "Page.enable goes first"
    );
}

/// The screencast scripts the dock's own session (S4) got, in order.
fn dock_scripts(setup: &Setup) -> Vec<Value> {
    setup
        .fake
        .sent("Page.addScriptToEvaluateOnNewDocument")
        .into_iter()
        .filter(|script| script["sessionId"] == "S4")
        .collect()
}

#[tokio::test]
async fn the_screencast_setting_turns_the_frontends_screencast_on() {
    let mut user = rb_config::UserSettings::default();
    user.devtools.screencast = true;
    let setup = setup_with(user);
    attach_views(&setup).await;
    open_prepared(&setup).await;
    let scripts = dock_scripts(&setup);
    assert_eq!(scripts.len(), 1, "{scripts:#?}");
    assert_eq!(
        scripts[0]["params"]["source"],
        screencast::preference_script(true)
    );
}

#[tokio::test]
async fn a_changed_screencast_setting_prepares_a_new_dock() {
    let setup = setup();
    attach_views(&setup).await;
    let before = open_prepared(&setup).await;
    let mut user = setup.core.user_settings();
    user.devtools.screencast = true;
    setup.core.replace_user_settings(user);
    setup.devtools.on_settings_changed().expect("applies");
    // The dock went and came back at its marker: the frontend reads the setting at its start.
    let since: Vec<_> = docks(&setup.ui).into_iter().skip(2).collect();
    assert_eq!(since[0], None, "{since:#?}");
    let marker = since[1].clone().expect("a new dock");
    assert_eq!(
        (marker.view, marker.frontend_url.as_str()),
        (ViewId(1), DOCK_MARKER_URL)
    );
    setup.devtools.on_dock_loaded(DOCK_MARKER_URL);
    until(|| docks(&setup.ui).len() == 5).await;
    let frontend = docks(&setup.ui)[4].clone().expect("the frontend");
    assert_ne!(frontend.frontend_url, before.frontend_url, "a new ticket");
    let last = setup
        .fake
        .sent("Page.addScriptToEvaluateOnNewDocument")
        .into_iter()
        .rfind(|script| script["sessionId"] == "S5")
        .expect("the new dock's script");
    assert_eq!(
        last["params"]["source"],
        screencast::preference_script(true)
    );
    // The same settings again change nothing.
    setup.devtools.on_settings_changed().expect("applies");
    assert_eq!(docks(&setup.ui).len(), 6, "only the redock");
}

#[tokio::test]
async fn a_view_without_a_target_cannot_be_inspected_docked() {
    let setup = setup();
    let refused = setup.devtools.open(ViewId(1), DevtoolsMode::DockedRight);
    assert!(matches!(refused, Err(EngineError::Invalid(_))));
    assert!(docks(&setup.ui).is_empty());
    assert_eq!(setup.core.host().shell_state().devtools, None);
}

#[tokio::test]
async fn docked_devtools_follow_the_active_view_within_a_second() {
    let setup = setup();
    attach_views(&setup).await;
    let before = open_prepared(&setup).await;
    setup
        .core
        .host()
        .set_active_view(ViewId(2))
        .expect("activates");
    until(|| docks(&setup.ui).len() == 3).await;
    let after = docks(&setup.ui)[2].clone().expect("a dock");
    assert_eq!(after.view, ViewId(2));
    assert_ne!(after.frontend_url, before.frontend_url, "a new ticket");
    assert_eq!(
        setup.devtools.current().map(|open| open.view),
        Some(ViewId(2))
    );
}

#[tokio::test]
async fn without_follow_the_devtools_stay_with_their_view() {
    let mut user = rb_config::UserSettings::default();
    user.devtools.follow_active_view = false;
    let setup = setup_with(user);
    attach_views(&setup).await;
    open_prepared(&setup).await;
    setup
        .core
        .host()
        .set_active_view(ViewId(3))
        .expect("activates");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(docks(&setup.ui).len(), 2);
    assert_eq!(
        setup.devtools.current().map(|open| open.view),
        Some(ViewId(1))
    );
}

#[tokio::test]
async fn the_window_mode_opens_the_native_window_and_its_close_is_remembered() {
    let setup = setup();
    attach_views(&setup).await;
    setup
        .devtools
        .open(ViewId(2), DevtoolsMode::Window)
        .expect("opens");
    assert!(setup.ui.calls().contains(&UiCall::Devtools(ViewId(2))));
    assert!(docks(&setup.ui).is_empty());
    let before = metrics_for(&setup, "S2");
    let bounds = WindowBounds {
        x: 40,
        y: 30,
        width: 1000,
        height: 700,
    };
    setup.devtools.on_window_closed(ViewId(2), Some(bounds));
    assert_eq!(setup.devtools.current(), None);
    assert_eq!(setup.core.host().shell_state().devtools, None);
    assert_eq!(setup.devtools.window_bounds(), Some(bounds));
    until(|| metrics_for(&setup, "S2") > before).await;
    // A new start reads the bounds back.
    let path = setup._dir.path().join("devtools.json");
    assert_eq!(ui_state::load(&path).window, Some(bounds));
}

#[tokio::test]
async fn switching_modes_moves_the_devtools_without_a_restart() {
    let setup = setup();
    attach_views(&setup).await;
    open_prepared(&setup).await;
    setup
        .devtools
        .open(ViewId(1), DevtoolsMode::DockedRight)
        .expect("docks right");
    let right = docks(&setup.ui).last().cloned().flatten().expect("a dock");
    assert_eq!((right.side, right.size), (DockSide::Right, 480));
    setup
        .devtools
        .open(ViewId(1), DevtoolsMode::Window)
        .expect("a window");
    assert_eq!(docks(&setup.ui).last(), Some(&None), "the dock went");
    assert!(setup.ui.calls().contains(&UiCall::Devtools(ViewId(1))));
    setup
        .devtools
        .open(ViewId(1), DevtoolsMode::DockedBottom)
        .expect("docks again");
    assert!(setup.ui.calls().contains(&UiCall::CloseDevtools(ViewId(1))));
    let again = docks(&setup.ui).last().cloned().flatten().expect("a dock");
    assert_eq!(
        again.frontend_url, DOCK_MARKER_URL,
        "a new dock prepares again"
    );
}

#[tokio::test]
async fn closing_after_the_frontend_was_connected_gives_the_view_its_emulation_back() {
    let setup = setup();
    attach_views(&setup).await;
    let dock = open_prepared(&setup).await;
    let (_, address) = dock.frontend_url.split_once("?ws=").expect("ws");
    let port: u16 = address
        .split_once(':')
        .and_then(|(_, rest)| rest.split('/').next())
        .and_then(|port| port.parse().ok())
        .expect("a port");
    let stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connects");
    let (mut ws, _) = tokio_tungstenite::client_async(format!("ws://{address}"), stream)
        .await
        .expect("admitted");
    // The device toolbar overrides the metrics on the frontend's own session.
    ws.send(Message::text(
        json!({ "id": 1, "method": "Emulation.setDeviceMetricsOverride", "params": { "width": 320, "height": 640, "deviceScaleFactor": 3, "mobile": true } })
            .to_string(),
    ))
    .await
    .expect("sends");
    until(|| metrics_for(&setup, "S5") == 1).await;
    let before = metrics_for(&setup, "S1");
    setup.devtools.close().expect("closes");
    assert_eq!(docks(&setup.ui).last(), Some(&None));
    assert_eq!(setup.core.host().shell_state().devtools, None);
    until(|| metrics_for(&setup, "S1") > before).await;
    let last = setup
        .fake
        .sent("Emulation.setDeviceMetricsOverride")
        .into_iter()
        .rev()
        .find(|message| message["sessionId"] == "S1")
        .expect("re-applied");
    assert_eq!(last["params"]["width"], 1440, "the breakpoint's own width");
    // The frontend's session and the dock's own one are detached.
    for session in ["S5", "S4"] {
        until(|| {
            setup
                .fake
                .sent("Target.detachFromTarget")
                .iter()
                .any(|message| message["params"]["sessionId"] == session)
        })
        .await;
    }
}

#[tokio::test]
async fn toggling_opens_for_the_active_view_and_closes_again() {
    let setup = setup();
    attach_views(&setup).await;
    setup
        .core
        .host()
        .set_active_view(ViewId(3))
        .expect("activates");
    let opened = setup.devtools.toggle().expect("toggles");
    assert_eq!(
        opened,
        Some(DevtoolsState {
            view: ViewId(3),
            mode: DevtoolsMode::DockedBottom
        })
    );
    assert_eq!(setup.devtools.toggle().expect("toggles"), None);
    assert_eq!(docks(&setup.ui).last(), Some(&None));
}

#[tokio::test]
async fn devtools_of_a_closed_view_close_too() {
    let setup = setup();
    attach_views(&setup).await;
    open_prepared(&setup).await;
    setup.devtools.on_view_closed(ViewId(2));
    assert!(setup.devtools.current().is_some());
    setup.devtools.on_view_closed(ViewId(1));
    assert_eq!(setup.devtools.current(), None);
}

/// Whether `session` was detached from its target.
fn detached(setup: &Setup, session: &str) -> bool {
    setup
        .fake
        .sent("Target.detachFromTarget")
        .iter()
        .any(|message| message["params"]["sessionId"] == session)
}

#[tokio::test]
async fn a_setting_toggled_while_the_marker_prepares_scripts_the_new_dock_not_the_stale_one() {
    let setup = setup();
    attach_views(&setup).await;
    let queries = setup.fake.sent("Target.getTargets").len();
    setup.fake.hold("Target.getTargets", None);
    setup
        .devtools
        .open(ViewId(1), DevtoolsMode::DockedBottom)
        .expect("opens");
    setup.devtools.on_dock_loaded(DOCK_MARKER_URL);
    until(|| setup.fake.sent("Target.getTargets").len() > queries).await;
    // The screencast setting changes while the marker's preparation waits on the browser: the
    // dock goes and a new one comes at its marker.
    let mut user = setup.core.user_settings();
    user.devtools.screencast = true;
    setup.core.replace_user_settings(user);
    setup.devtools.on_settings_changed().expect("applies");
    let since: Vec<_> = docks(&setup.ui).into_iter().skip(1).collect();
    assert_eq!(since[0], None, "{since:#?}");
    assert_eq!(
        since[1].as_ref().map(|dock| dock.frontend_url.as_str()),
        Some(DOCK_MARKER_URL),
        "{since:#?}"
    );
    // The stale preparation finishes: its session goes, and no frontend loads from it.
    setup.fake.release();
    until(|| detached(&setup, "S4")).await;
    assert_eq!(docks(&setup.ui).len(), 3, "{:#?}", docks(&setup.ui));
    let stale = dock_scripts(&setup);
    assert_eq!(stale.len(), 1, "{stale:#?}");
    assert_eq!(
        stale[0]["params"]["source"],
        screencast::preference_script(false)
    );
    // The new dock's marker loads: it is prepared for itself (its target hidden, the script of
    // the setting of now), and only then does it get the frontend.
    setup.devtools.on_dock_loaded(DOCK_MARKER_URL);
    until(|| docks(&setup.ui).len() == 4).await;
    let frontend = docks(&setup.ui)[3].clone().expect("the frontend");
    assert!(
        frontend.frontend_url.starts_with("devtools://"),
        "{frontend:#?}"
    );
    let script = setup
        .fake
        .sent("Page.addScriptToEvaluateOnNewDocument")
        .into_iter()
        .rfind(|script| script["sessionId"] == "S5")
        .expect("the new dock's script");
    assert_eq!(
        script["params"]["source"],
        screencast::preference_script(true)
    );
    assert!(!detached(&setup, "S5"), "the new dock's session stays");
}
