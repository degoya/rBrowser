use serde_json::json;

use super::*;

fn answer(status: u16, body: serde_json::Value) -> Answer {
    Answer { status, body }
}

#[test]
fn a_success_is_its_value() {
    let value = outcome("navigate", answer(200, json!({ "value": null }))).expect("ok");
    assert_eq!(value, serde_json::Value::Null);
    let value = outcome("url", answer(200, json!({ "value": "http://a/" }))).expect("ok");
    assert_eq!(value, json!("http://a/"));
}

#[test]
fn an_error_names_the_command_and_webdriver_s_code_without_the_stack() {
    let body = json!({ "value": {
        "error": "no such element",
        "message": "Unable to locate an element",
        "stacktrace": "secret frames",
    } });
    let error = outcome("find element", answer(404, body)).expect_err("refused");
    assert_eq!(
        error,
        SafariError::Command {
            command: "find element".to_owned(),
            error: "no such element".to_owned(),
            message: "Unable to locate an element".to_owned(),
        }
    );
    assert!(!error.to_string().contains("secret"));
}

#[test]
fn a_long_message_is_cut() {
    let body = json!({ "value": { "error": "javascript error", "message": "x".repeat(5000) } });
    let SafariError::Command { message, .. } =
        outcome("execute", answer(500, body)).expect_err("refused")
    else {
        panic!("a command error");
    };
    assert!(
        message.chars().count() <= MAX_MESSAGE + 3,
        "{}",
        message.len()
    );
}

#[test]
fn an_error_without_webdriver_s_shape_is_still_an_error() {
    let error = outcome("status", answer(500, json!("oops"))).expect_err("refused");
    assert!(matches!(error, SafariError::Command { ref error, .. } if error == "unknown error"));
}

#[test]
fn remote_automation_switched_off_is_recognised() {
    // The texts safaridriver is known to answer; the spike on a Mac confirms them (RB-095-03).
    for message in [
        "Could not create a session: You must enable the 'Allow Remote Automation' option in Safari's Develop menu to control Safari via WebDriver.",
        "safaridriver is not enabled; run safaridriver --enable once",
        "Remote automation is turned off",
    ] {
        let body = json!({ "value": { "error": "session not created", "message": message } });
        assert_eq!(
            outcome("new session", answer(500, body)),
            Err(SafariError::NotEnabled),
            "{message}"
        );
    }
}

#[test]
fn a_second_session_is_recognised() {
    for message in [
        "Could not create a session: The Safari instance is already paired with another WebDriver session.",
        "Could not create a session: Safari is already being automated by another session",
    ] {
        let body = json!({ "value": { "error": "session not created", "message": message } });
        assert_eq!(
            outcome("new session", answer(500, body)),
            Err(SafariError::SessionBusy),
            "{message}"
        );
    }
}

#[test]
fn another_failed_session_stays_a_command_error() {
    let body = json!({ "value": { "error": "session not created", "message": "Safari crashed" } });
    assert!(matches!(
        outcome("new session", answer(500, body)),
        Err(SafariError::Command { .. })
    ));
}

#[test]
fn an_element_reference_is_read_from_its_w3c_key() {
    let value = json!({ ELEMENT_KEY: "node-7" });
    assert_eq!(element_id(&value), Some("node-7".to_owned()));
    assert_eq!(element_id(&json!({ "ELEMENT": "x" })), None);
    assert_eq!(element_id(&json!({ ELEMENT_KEY: "a b" })), None);
}
