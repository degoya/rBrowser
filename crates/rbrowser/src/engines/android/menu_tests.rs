use std::sync::Arc;

use rb_config::AndroidSettings;
use rb_ipc::{AndroidDeviceRow, AndroidDeviceState};

use super::*;
use crate::engines::android::adb::tests::FakeAdb;
use crate::engines::android::adb::{AdbError, AdbRun};
use crate::engines::android::engine::{AdbFactory, AndroidEngine};
use crate::engines::android::forward::devices_args;
use crate::engines::android::locate::Env;

fn device(serial: &str, state: DeviceState, model: Option<&str>) -> Device {
    Device {
        serial: serial.to_owned(),
        state,
        model: model.map(ToOwned::to_owned),
        product: None,
    }
}

/// An adb program on disk (never run: the fake answers) and the engine over the fake.
fn engine(enabled: bool, adb: &Arc<FakeAdb>) -> (tempfile::TempDir, Arc<AndroidEngine>) {
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
    let engine = AndroidEngine::with(settings, None, Env::default(), factory, Vec::new);
    (dir, engine)
}

#[test]
fn each_device_is_a_row_with_its_name_and_state() {
    let devices = [
        device("R5CX", DeviceState::Device, Some("SM-S938B")),
        device("EMU1", DeviceState::Unauthorized, None),
        device("OFF1", DeviceState::Offline, None),
        device("PERM", DeviceState::NoPermissions, None),
        device("BOOT", DeviceState::Other("bootloader".to_owned()), None),
    ];
    let menu = menu_of(&devices, None);
    assert_eq!(
        menu.android[0],
        AndroidDeviceRow {
            serial: "R5CX".to_owned(),
            name: "SM-S938B".to_owned(),
            state: AndroidDeviceState::Ready,
        }
    );
    let states: Vec<AndroidDeviceState> = menu.android.iter().map(|row| row.state).collect();
    assert_eq!(
        states,
        [
            AndroidDeviceState::Ready,
            AndroidDeviceState::Unauthorized,
            AndroidDeviceState::Offline,
            AndroidDeviceState::NoPermissions,
            AndroidDeviceState::Other,
        ]
    );
    assert_eq!(menu.android[1].name, "EMU1", "no model: the serial");
    assert_eq!(menu.android_reason, None);
    assert_eq!(menu.apple, cfg!(target_os = "macos"));
}

#[tokio::test]
async fn the_list_is_what_adb_answers() {
    let adb = Arc::new(FakeAdb::default());
    adb.answer(
        &devices_args(),
        Ok("List of devices attached\nR5CX device model:SM_S938B\n"),
    );
    let (_dir, on) = engine(true, &adb);
    let menu = list(&on).await;
    assert_eq!(menu.android.len(), 1);
    assert_eq!(menu.android[0].name, "SM S938B");
    assert_eq!(menu.android_reason, None);
}

#[tokio::test]
async fn off_lists_nothing_and_runs_no_adb() {
    let adb = Arc::new(FakeAdb::default());
    let (_dir, off) = engine(false, &adb);
    let menu = list(&off).await;
    assert!(menu.android.is_empty());
    assert_eq!(menu.android_reason, None);
    assert!(adb.calls().is_empty(), "{:?}", adb.calls());
}

#[tokio::test]
async fn an_adb_that_fails_is_the_reason_of_the_empty_list() {
    let adb = Arc::new(FakeAdb::default());
    adb.answer(&devices_args(), Err(AdbError::Timeout));
    let (_dir, on) = engine(true, &adb);
    let menu = list(&on).await;
    assert!(menu.android.is_empty());
    assert_eq!(
        menu.android_reason.as_deref(),
        Some(AdbError::Timeout.to_string().as_str())
    );
}

#[test]
fn one_listing_runs_at_a_time() {
    let first = Listing::start().expect("free");
    assert!(Listing::start().is_none(), "the first one still runs");
    drop(first);
    assert!(Listing::start().is_some(), "free again");
}
