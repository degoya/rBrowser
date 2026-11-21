use std::sync::Arc;

use rb_config::{AndroidSettings, Breakpoint, EmulationFields, EngineKind};
use rb_engine::{Engine, EngineError, EngineHealth, ViewSpec};

use super::*;
use crate::engines::android::adb::tests::FakeAdb;
use crate::engines::android::devices::breakpoint_id;
use crate::engines::android::fake_chrome;
use crate::engines::android::forward::{devices_args, forward_args, remove_args};
use crate::engines::android::reverse::{reverse_args, reverse_list_args, unreverse_args};

/// The machine's own address the tests' exposure check asks at (TEST-NET-1: nothing answers).
fn test_net() -> Vec<std::net::IpAddr> {
    vec![std::net::IpAddr::from([192, 0, 2, 77])]
}

/// An adb program on disk (never run: the fake answers) and the engine over the fake.
fn engine(enabled: bool, adb: &Arc<FakeAdb>) -> (tempfile::TempDir, Arc<AndroidEngine>) {
    engine_at(enabled, adb, test_net)
}

/// [`engine`] with `own_addresses` for the exposure check.
fn engine_at(
    enabled: bool,
    adb: &Arc<FakeAdb>,
    own_addresses: fn() -> Vec<std::net::IpAddr>,
) -> (tempfile::TempDir, Arc<AndroidEngine>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let program = dir.path().join("adb");
    std::fs::write(&program, b"not run").expect("a file");
    let settings = AndroidSettings {
        enabled,
        adb_path: Some(program.display().to_string()),
        ..AndroidSettings::default()
    };
    let fake = adb.clone();
    let factory: AdbFactory = Arc::new(move |_found| fake.clone() as Arc<dyn AdbRun>);
    let engine = AndroidEngine::with(settings, None, Env::default(), factory, own_addresses);
    (dir, engine)
}

fn spec(serial: &str) -> ViewSpec {
    let id = breakpoint_id(serial);
    ViewSpec {
        view: Some(ViewId(9)),
        breakpoint: Breakpoint {
            id: id.clone(),
            label: Some(id),
            width: Some(412),
            height: Some(915),
            device: None,
            engine: EngineKind::Android,
            ios_device: None,
            session: None,
            sync_navigation: true,
            shortcut: None,
            color: None,
            emulation: EmulationFields::default(),
        },
        emulation: rb_engine::Emulation::default(),
        url: "https://example.test/".to_owned(),
        title: String::new(),
        window: None,
        cell: None,
    }
}

fn reason(error: EngineError) -> String {
    match error {
        EngineError::Unavailable { engine, reason } => {
            assert_eq!(engine, EngineKind::Android);
            reason
        }
        other => panic!("not unavailable: {other}"),
    }
}

/// Off by default: the engine says so, and nothing runs adb.
#[tokio::test]
async fn switched_off_the_engine_is_unavailable_and_runs_nothing() {
    let adb = Arc::new(FakeAdb::default());
    let (_dir, engine) = engine(false, &adb);
    let disabled = rb_i18n::t("engines.android.disabled");
    assert_eq!(engine.health(), EngineHealth::Unavailable(disabled.clone()));
    let error = engine
        .create_view(&spec("S1"))
        .await
        .err()
        .expect("refused");
    assert_eq!(reason(error), disabled);
    assert!(adb.calls().is_empty());
}

/// Without adb the reason names the setting that names no file.
#[tokio::test]
async fn an_adb_path_that_is_no_file_is_the_reason() {
    let adb = Arc::new(FakeAdb::default());
    let (dir, engine) = engine(true, &adb);
    let missing = dir.path().join("nothing").display().to_string();
    engine.set_settings(AndroidSettings {
        enabled: true,
        adb_path: Some(missing.clone()),
        ..AndroidSettings::default()
    });
    let EngineHealth::Unavailable(text) = engine.health() else {
        panic!("unavailable");
    };
    assert_eq!(
        text,
        rb_i18n::t_with("engines.android.adbPathNoFile", &[("path", &missing)])
    );
}

#[tokio::test]
async fn a_device_not_connected_or_not_authorised_is_the_reason() {
    let adb = Arc::new(FakeAdb::default());
    adb.answer(
        &devices_args(),
        Ok("List of devices attached\nS2 unauthorized usb:1\n"),
    );
    let (_dir, engine) = engine(true, &adb);
    assert_eq!(engine.health(), EngineHealth::Stopped);
    let error = engine
        .create_view(&spec("S1"))
        .await
        .err()
        .expect("refused");
    assert_eq!(
        reason(error),
        rb_i18n::t_with("engines.android.notConnected", &[("serial", "S1")])
    );
    let error = engine
        .create_view(&spec("S2"))
        .await
        .err()
        .expect("refused");
    assert_eq!(
        reason(error),
        rb_i18n::t_with("engines.android.unauthorized", &[("serial", "S2")])
    );
    assert!(
        !adb.calls().iter().any(|call| call.contains("forward")),
        "{:?}",
        adb.calls()
    );
}

/// The whole way: the device, the forward to 127.0.0.1, Chrome's socket named with another
/// host but dialled on loopback, the tab; shutting down closes the tab and the forward.
#[tokio::test]
async fn a_ready_device_opens_a_view_over_the_loopback_forward() {
    let chrome = fake_chrome::start().await;
    let adb = Arc::new(FakeAdb::default());
    adb.answer(
        &devices_args(),
        Ok("List of devices attached\nS1 device model:Pixel_8\n"),
    );
    adb.answer(&forward_args("S1"), Ok(&format!("{}\n", chrome.port)));
    adb.answer(&remove_args("S1", chrome.port), Ok(""));
    let (_dir, engine) = engine(true, &adb);
    let view = engine.create_view(&spec("S1")).await.expect("opened");
    assert_eq!(view.engine(), EngineKind::Android);
    assert!(engine.view(ViewId(9)).is_some());
    assert_eq!(engine.health(), EngineHealth::Running);
    assert_eq!(
        chrome.methods(),
        [
            "Target.createTarget",
            "Target.attachToTarget",
            "Page.enable"
        ]
    );
    engine.shutdown().await;
    assert!(engine.view(ViewId(9)).is_none());
    assert!(
        adb.calls()
            .contains(&remove_args("S1", chrome.port).join(" "))
    );
}

/// No own address to ask the forward at proves nothing: the view stays closed, the forward goes.
#[tokio::test]
async fn a_forward_with_no_address_to_check_it_at_is_refused() {
    let chrome = fake_chrome::start().await;
    let adb = Arc::new(FakeAdb::default());
    adb.answer(
        &devices_args(),
        Ok("List of devices attached\nS1 device model:Pixel_8\n"),
    );
    adb.answer(&forward_args("S1"), Ok(&format!("{}\n", chrome.port)));
    adb.answer(&remove_args("S1", chrome.port), Ok(""));
    let (_dir, engine) = engine_at(true, &adb, Vec::new);
    let Err(EngineError::Unavailable { reason, .. }) = engine.create_view(&spec("S1")).await else {
        panic!("refused");
    };
    assert!(
        reason.contains("network address") || reason == "engines.android.unchecked",
        "{reason}"
    );
    assert!(chrome.methods().is_empty(), "Chrome was not asked");
    assert!(
        adb.calls()
            .contains(&remove_args("S1", chrome.port).join(" "))
    );
}

#[test]
fn every_device_state_has_its_reason() {
    let devices = crate::engines::android::devices::parse_devices(
        "List of devices attached\nA offline\nB no permissions (udev)\nC recovery\nD device\n",
    );
    assert!(ready_device("android-A", &devices).is_err());
    assert!(ready_device("android-B", &devices).is_err());
    assert!(ready_device("android-C", &devices).is_err());
    assert_eq!(
        ready_device("android-D", &devices).map(|device| device.serial.as_str()),
        Ok("D")
    );
}

/// RB-096-12: a view opened at localhost reverses its port; switching the setting off (as
/// `on_config_changed` does) takes every reverse away and lets none be set again.
#[tokio::test]
async fn switching_the_reverses_off_removes_them() {
    let chrome = fake_chrome::start().await;
    let adb = Arc::new(FakeAdb::default());
    adb.answer(
        &devices_args(),
        Ok("List of devices attached\nS1 device model:Pixel_8\n"),
    );
    adb.answer(&forward_args("S1"), Ok(&format!("{}\n", chrome.port)));
    adb.answer(&remove_args("S1", chrome.port), Ok(""));
    adb.answer(&reverse_list_args("S1"), Ok(""));
    adb.answer(&reverse_args("S1", 3000), Ok(""));
    adb.answer(&unreverse_args("S1", 3000), Ok(""));
    let (dir, engine) = engine(true, &adb);
    assert!(engine.reverses_wanted(), "on by default");
    let mut at_localhost = spec("S1");
    at_localhost.url = "http://localhost:3000/".to_owned();
    let view = engine.create_view(&at_localhost).await.expect("opened");
    assert!(
        adb.calls().contains(&reverse_args("S1", 3000).join(" ")),
        "{:?}",
        adb.calls()
    );
    engine.set_settings(AndroidSettings {
        enabled: true,
        adb_path: Some(dir.path().join("adb").display().to_string()),
        reverse_localhost: false,
    });
    assert!(!engine.reverses_wanted());
    engine.clear_reverses().await;
    assert!(
        adb.calls().contains(&unreverse_args("S1", 3000).join(" ")),
        "{:?}",
        adb.calls()
    );
    view.navigate("http://localhost:3000/again")
        .await
        .expect("navigates");
    let set = reverse_args("S1", 3000).join(" ");
    assert_eq!(adb.calls().iter().filter(|call| **call == set).count(), 1);
}

/// The app quits (`Engine::shutdown`): each view closes, its reverses first.
#[tokio::test]
async fn shutting_down_removes_the_reverses() {
    let chrome = fake_chrome::start().await;
    let adb = Arc::new(FakeAdb::default());
    adb.answer(
        &devices_args(),
        Ok("List of devices attached\nS1 device model:Pixel_8\n"),
    );
    adb.answer(&forward_args("S1"), Ok(&format!("{}\n", chrome.port)));
    adb.answer(&remove_args("S1", chrome.port), Ok(""));
    adb.answer(&reverse_list_args("S1"), Ok(""));
    adb.answer(&reverse_args("S1", 3000), Ok(""));
    adb.answer(&unreverse_args("S1", 3000), Ok(""));
    let (_dir, engine) = engine(true, &adb);
    let mut at_localhost = spec("S1");
    at_localhost.url = "http://127.0.0.1:3000/".to_owned();
    engine.create_view(&at_localhost).await.expect("opened");
    engine.shutdown().await;
    let calls = adb.calls();
    let at = |args: &[String]| calls.iter().position(|call| *call == args.join(" "));
    let unreversed = at(&unreverse_args("S1", 3000));
    assert!(unreversed.is_some(), "{calls:?}");
    // The security review S2: the reverse goes before anything else of the view.
    assert!(
        unreversed < at(&remove_args("S1", chrome.port)),
        "{calls:?}"
    );
}
