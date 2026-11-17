use serde_json::json;

use super::*;

#[test]
fn the_preload_runs_in_the_sandbox_with_a_channel() {
    let command = add_preload_script("(emit) => {}", "rb-agent-3", &["c1"]);
    assert_eq!(command.method, "script.addPreloadScript");
    assert_eq!(
        command.params,
        json!({
            "functionDeclaration": "(emit) => {}",
            "arguments": [{ "type": "channel", "value": { "channel": "rb-agent-3" } }],
            "sandbox": "rbrowser",
            "contexts": ["c1"],
        })
    );
}

#[test]
fn evaluate_targets_the_sandbox_or_the_page() {
    assert_eq!(
        evaluate("c1", true, "1+1").params,
        json!({
            "expression": "1+1",
            "target": { "context": "c1", "sandbox": "rbrowser" },
            "awaitPromise": false,
            "resultOwnership": "none",
        })
    );
    assert_eq!(
        evaluate("c1", false, "1").params["target"],
        json!({ "context": "c1" })
    );
}

#[test]
fn a_string_result_is_read_and_an_exception_is_not_quoted() {
    let ok = json!({ "type": "success", "result": { "type": "string", "value": "{\"a\":1}" }, "realm": "r" });
    assert_eq!(string_result(&ok), Ok("{\"a\":1}".to_owned()));
    let thrown = json!({ "type": "exception", "exceptionDetails": { "text": "ReferenceError: hunter2" }, "realm": "r" });
    let error = string_result(&thrown).expect_err("thrown");
    assert!(!error.to_string().contains("hunter2"), "{error}");
    let number = json!({ "type": "success", "result": { "type": "number", "value": 1 } });
    assert!(string_result(&number).is_err());
}

#[test]
fn a_channel_message_names_channel_context_and_text() {
    let params = json!({
        "channel": "rb-agent-3",
        "data": { "type": "string", "value": "{\"kind\":\"ready\"}" },
        "source": { "realm": "r1", "context": "c1" },
    });
    assert_eq!(
        channel_message(MESSAGE_EVENT, &params),
        Some(ChannelMessage {
            channel: "rb-agent-3".to_owned(),
            context: "c1".to_owned(),
            data: "{\"kind\":\"ready\"}".to_owned(),
        })
    );
    let object = json!({ "channel": "rb-agent-3", "data": { "type": "object", "value": [] }, "source": { "context": "c1" } });
    assert_eq!(channel_message(MESSAGE_EVENT, &object), None);
    assert_eq!(channel_message("log.entryAdded", &params), None);
}
