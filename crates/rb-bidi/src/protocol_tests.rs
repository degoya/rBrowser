use serde_json::json;

use super::*;

#[test]
fn a_command_frame_carries_id_method_and_params() {
    let frame = command_frame(7, "browsingContext.navigate", json!({ "url": "http://x/" }));
    let value: Value = serde_json::from_str(&frame).expect("json");
    assert_eq!(
        value,
        json!({ "id": 7, "method": "browsingContext.navigate", "params": { "url": "http://x/" } })
    );
}

#[test]
fn a_success_answers_its_command() {
    let incoming = parse_incoming(r#"{"type":"success","id":3,"result":{"context":"c1"}}"#);
    assert_eq!(
        incoming,
        Ok(Incoming::Answer {
            id: 3,
            outcome: Ok(json!({ "context": "c1" })),
        })
    );
}

#[test]
fn an_error_keeps_the_error_code_and_message() {
    let incoming = parse_incoming(
        r#"{"type":"error","id":4,"error":"no such frame","message":"Browsing context x not found","stacktrace":"..."}"#,
    );
    assert_eq!(
        incoming,
        Ok(Incoming::Answer {
            id: 4,
            outcome: Err(Failure {
                error: "no such frame".to_owned(),
                message: "Browsing context x not found".to_owned(),
            }),
        })
    );
}

#[test]
fn an_error_without_id_is_a_protocol_complaint() {
    let incoming =
        parse_incoming(r#"{"type":"error","id":null,"error":"invalid argument","message":"x"}"#);
    assert_eq!(
        incoming,
        Ok(Incoming::Unrouted(Failure {
            error: "invalid argument".to_owned(),
            message: "x".to_owned(),
        }))
    );
}

#[test]
fn an_event_has_method_and_params() {
    let incoming = parse_incoming(
        r#"{"type":"event","method":"script.message","params":{"channel":"rb-agent-1","data":{"type":"string","value":"{}"}}}"#,
    );
    assert_eq!(
        incoming,
        Ok(Incoming::Event(Event {
            method: "script.message".to_owned(),
            params: json!({ "channel": "rb-agent-1", "data": { "type": "string", "value": "{}" } }),
        }))
    );
}

#[test]
fn garbage_is_refused_without_quoting_it() {
    let secret = "{\"type\":\"success\",\"id\":\"hunter2\"}";
    let error = parse_incoming(secret).expect_err("a string id is not BiDi");
    assert!(!error.contains("hunter2"), "{error}");
    let error = parse_incoming("not json at all hunter2").expect_err("not json");
    assert!(!error.contains("hunter2"), "{error}");
    assert!(parse_incoming(r#"{"type":"surprise"}"#).is_err());
}

#[test]
fn a_string_remote_value_is_read_and_others_are_not() {
    assert_eq!(
        string_value(&json!({ "type": "string", "value": "hi" })),
        Some("hi")
    );
    assert_eq!(string_value(&json!({ "type": "number", "value": 3 })), None);
}

#[test]
fn a_command_never_shows_its_parameters() {
    let command = Command::call(
        "storage.setCookie",
        json!({ "cookie": { "value": { "type": "string", "value": "hunter2" } } }),
    );
    let shown = format!("{command:?}");
    assert!(shown.contains("storage.setCookie"), "{shown}");
    assert!(!shown.contains("hunter2"), "{shown}");
}
