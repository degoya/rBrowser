use serde_json::json;

use super::*;
use crate::WslError;
use crate::netmode::{NetworkingMode, resolve};

#[test]
fn the_arguments_default_to_info_and_refuse_strangers() {
    let args: WslInfoArgs = serde_json::from_value(json!({})).expect("reads");
    assert_eq!(args.action, WslInfoAction::Info);
    let args: WslInfoArgs = serde_json::from_value(json!({"action": "restart"})).expect("reads");
    assert_eq!(args.action, WslInfoAction::Restart);
    assert!(serde_json::from_value::<WslInfoArgs>(json!({"grant": true})).is_err());
}

#[test]
fn the_answer_as_json() {
    let info = WslInfo::new(
        Vec::new(),
        Some("Ubuntu".into()),
        Some(resolve(None, Some(NetworkingMode::Mirrored), None)),
        HelperStatus::Stopped,
        None,
    );
    assert_eq!(
        serde_json::to_value(&info).expect("json"),
        json!({
            "distros": [],
            "distro": "Ubuntu",
            "networking": {"mode": "mirrored", "source": "wslinfo"},
            "reachesWindowsLocalhost": true,
            "helper": {"state": "stopped"},
            "error": null
        })
    );
}

#[test]
fn an_unavailable_wsl_names_its_key() {
    let info = WslInfo::new(
        Vec::new(),
        None,
        None,
        HelperStatus::Stopped,
        Some(&WslError::NotInstalled),
    );
    let error = info.error.expect("an error");
    assert_eq!(error.key, "wsl.error.notInstalled");
    assert_eq!(info.reaches_windows_localhost, None);
}
