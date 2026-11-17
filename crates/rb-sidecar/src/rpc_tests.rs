use serde_json::{Value, json};

use super::*;

#[test]
fn a_request_is_one_line_without_null_params() {
    let line = request_line(7, "sidecar.ping", &Value::Null).expect("encodes");
    assert_eq!(
        line,
        "{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"sidecar.ping\"}\n"
    );
}

#[test]
fn a_request_with_a_newline_in_its_params_stays_one_line() {
    let line = request_line(1, "x", &json!({ "text": "a\nb" })).expect("encodes");
    assert_eq!(line.matches('\n').count(), 1);
    assert!(line.ends_with('\n'));
}

#[test]
fn a_cancel_has_no_id() {
    let line = notification_line(CANCEL, &json!({ "id": 3 })).expect("encodes");
    assert_eq!(
        line,
        "{\"jsonrpc\":\"2.0\",\"method\":\"$/cancel\",\"params\":{\"id\":3}}\n"
    );
}

#[test]
fn an_answer_with_a_result_parses() {
    let incoming = parse_line(r#"{"jsonrpc":"2.0","id":4,"result":{"pong":true}}"#);
    assert_eq!(
        incoming.expect("parses"),
        Incoming::Response {
            id: 4,
            outcome: Ok(json!({ "pong": true }))
        }
    );
}

#[test]
fn an_answer_with_null_result_parses() {
    let incoming = parse_line(r#"{"jsonrpc":"2.0","id":4,"result":null}"#);
    assert_eq!(
        incoming.expect("parses"),
        Incoming::Response {
            id: 4,
            outcome: Ok(Value::Null)
        }
    );
}

#[test]
fn an_error_answer_parses() {
    let incoming = parse_line(
        r#"{"jsonrpc":"2.0","id":5,"error":{"code":-32601,"message":"unknown method x"}}"#,
    );
    assert_eq!(
        incoming.expect("parses"),
        Incoming::Response {
            id: 5,
            outcome: Err(ErrorObject {
                code: code::METHOD_NOT_FOUND,
                message: "unknown method x".to_owned(),
                data: None
            })
        }
    );
}

#[test]
fn a_progress_notification_parses() {
    let incoming =
        parse_line(r#"{"jsonrpc":"2.0","method":"$/progress","params":{"id":2,"percent":40}}"#);
    assert_eq!(
        incoming.expect("parses"),
        Incoming::Notification(Notification {
            method: PROGRESS.to_owned(),
            params: json!({ "id": 2, "percent": 40 })
        })
    );
}

#[test]
fn an_unaddressed_error_names_the_sidecars_message() {
    let error =
        parse_line(r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"not JSON"}}"#)
            .expect_err("unaddressed");
    assert_eq!(
        error.to_string(),
        "sidecar protocol error: the sidecar could not read a request: not JSON"
    );
}

#[test]
fn broken_messages_are_protocol_errors() {
    for text in [
        "not json",
        "[1]",
        r#"{"jsonrpc":"1.0","id":1,"result":1}"#,
        r#"{"jsonrpc":"2.0","id":1}"#,
        r#"{"jsonrpc":"2.0","id":"a","result":1}"#,
        r#"{"jsonrpc":"2.0","id":1,"method":"x"}"#,
        r#"{"jsonrpc":"2.0","id":1,"error":"x"}"#,
    ] {
        assert!(
            matches!(parse_line(text), Err(SidecarError::Protocol(_))),
            "{text}"
        );
    }
}
