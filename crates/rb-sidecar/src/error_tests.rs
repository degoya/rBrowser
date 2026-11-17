use std::path::PathBuf;

use super::*;

#[test]
fn a_wrong_major_names_both_versions_and_links_the_node_setting() {
    let error = SidecarError::NodeVersionMismatch {
        expected: 26,
        found: "24.21.0".to_owned(),
        path: PathBuf::from("C:/node/node.exe"),
    };
    assert_eq!(
        error.to_string(),
        "Node 26 expected, 24.21.0 found (C:/node/node.exe)"
    );
    assert_eq!(error.i18n_key(), "sidecar.error.nodeVersionMismatch");
    assert_eq!(
        error.i18n_args(),
        vec![
            ("expected", "26".to_owned()),
            ("found", "24.21.0".to_owned())
        ]
    );
    assert_eq!(error.settings_key(), Some("engines.nodePath"));
}

#[test]
fn a_timeout_links_the_timeout_setting() {
    let error = SidecarError::Timeout {
        method: "sidecar.ping".to_owned(),
        ms: 30_000,
    };
    assert_eq!(error.settings_key(), Some("engines.nodeRpcTimeoutMs"));
    assert_eq!(error.i18n_key(), "sidecar.error.timeout");
}

#[test]
fn a_remote_message_is_redacted_in_the_placeholders() {
    let error = SidecarError::Remote {
        method: "x".to_owned(),
        code: -32603,
        message: "failed with password=hunter2hunter2".to_owned(),
        data: None,
    };
    let args = error.i18n_args();
    assert!(!args.iter().any(|(_, value)| value.contains("hunter2")));
    assert_eq!(error.settings_key(), None);
}

#[test]
fn every_error_has_a_sidecar_error_key() {
    let errors = [
        SidecarError::NodeNotFound(PathBuf::new()),
        SidecarError::Exited,
        SidecarError::CrashLoop { crashes: 5 },
        SidecarError::Protocol(String::new()),
        SidecarError::BrowsersPath(String::new()),
        SidecarError::Unhealthy(String::new()),
        SidecarError::ShutDown,
    ];
    for error in errors {
        assert!(error.i18n_key().starts_with("sidecar.error."));
    }
}
