use super::*;

const UDID: &str = "8B1E5A9C-2F3D-4C11-9E77-0A1B2C3D4E5F";

#[test]
fn the_command_lines_name_the_one_simulator() {
    assert_eq!(boot_args(UDID), ["simctl", "boot", UDID]);
    assert_eq!(bootstatus_args(UDID), ["simctl", "bootstatus", UDID]);
    assert_eq!(
        show_args(UDID),
        ["-a", "Simulator", "--args", "-CurrentDeviceUDID", UDID]
    );
    assert_eq!(FIND_SIMCTL, ["--find", "simctl"]);
    assert!(LIST_SIMULATORS.contains(&"--json"));
    assert!(LIST_USB.contains(&"SPUSBHostDataType"));
}

#[tokio::test]
async fn outside_macos_there_is_no_ios() {
    if cfg!(target_os = "macos") {
        return;
    }
    let found = inventory().await;
    assert!(!found.xcode);
    assert!(found.simulators.is_empty() && found.devices.is_empty());
}
