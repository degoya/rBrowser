use rb_engine::{EngineError, EngineKind};

use super::*;

#[test]
fn a_gone_driver_is_a_closed_engine() {
    let error = SafariError::Connect("connection refused".to_owned());
    assert!(error.driver_gone());
    assert!(matches!(engine_error(error), EngineError::Closed));
}

#[test]
fn automation_switched_off_is_an_unavailable_engine_with_the_command_to_run() {
    let EngineError::Unavailable { engine, reason } = engine_error(SafariError::NotEnabled) else {
        panic!("unavailable");
    };
    assert_eq!(engine, EngineKind::Safari);
    // The catalogue's text, or its key while the part is not merged: either names it.
    assert!(
        reason.contains("safaridriver --enable") || reason == "engines.safari.notEnabled",
        "{reason}"
    );
}

#[test]
fn a_second_session_is_unavailable_not_a_crash() {
    assert!(matches!(
        engine_error(SafariError::SessionBusy),
        EngineError::Unavailable {
            engine: EngineKind::Safari,
            ..
        }
    ));
}

#[test]
fn an_unknown_command_is_a_missing_feature() {
    let error = SafariError::Command {
        command: "perform actions".to_owned(),
        error: "unknown command".to_owned(),
        message: String::new(),
    };
    assert!(error.unsupported());
    assert!(matches!(
        engine_error(error),
        EngineError::Unsupported {
            engine: EngineKind::Safari,
            ..
        }
    ));
}

#[test]
fn other_failures_are_invalid_and_name_the_command() {
    let error = SafariError::Timeout {
        command: "navigate".to_owned(),
        ms: 5,
    };
    let EngineError::Invalid(text) = engine_error(error) else {
        panic!("invalid");
    };
    assert!(text.contains("navigate"), "{text}");
}

#[test]
fn ios_names_its_own_engine_and_the_switch_on_the_device() {
    let EngineError::Unavailable { engine, reason } =
        engine_error_for(EngineKind::Ios, SafariError::NotEnabled)
    else {
        panic!("unavailable");
    };
    assert_eq!(engine, EngineKind::Ios);
    assert!(
        reason.contains("Remote Automation") || reason == "engines.ios.notEnabled",
        "{reason}"
    );
    assert!(matches!(
        engine_error_for(EngineKind::Ios, SafariError::Launch("no boot".to_owned())),
        EngineError::Unavailable {
            engine: EngineKind::Ios,
            ..
        }
    ));
    assert!(matches!(
        engine_error_for(EngineKind::Ios, SafariError::SessionBusy),
        EngineError::Unavailable {
            engine: EngineKind::Ios,
            ..
        }
    ));
}
