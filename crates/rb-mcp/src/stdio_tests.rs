use serde_json::{Value, json};

use super::{StdioError, answers_of, forward, sse_data};

#[test]
fn sse_data_takes_every_event() {
    let body = "event: message\ndata: {\"a\":1}\n\nid: 2\ndata: {\"b\":\ndata: 2}\n\n";
    assert_eq!(
        sse_data(body),
        vec!["{\"a\":1}".to_owned(), "{\"b\":\n2}".to_owned()]
    );
    assert!(sse_data(": ping\n\n").is_empty());
}

/// Without an instance the client still connects: `initialize` succeeds, a notification gets
/// nothing, a tool call gets the error that says what to do, and stdout holds JSON-RPC alone.
/// The input ended without an instance ever reached: the reason comes back for the exit code.
#[test]
fn without_an_instance_initialize_succeeds_and_tool_calls_say_what_to_do() {
    let input = [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": "navigate", "arguments": {} } }),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    let mut searches = 0;
    let mut output = Vec::new();
    let ended = forward(
        || {
            searches += 1;
            Err(StdioError::NoInstance("`/home/me/site`".to_owned()))
        },
        input.as_bytes(),
        &mut output,
    );
    assert!(matches!(ended, Err(StdioError::NoInstance(_))));
    assert_eq!(searches, 3, "every message looks again");
    let lines: Vec<Value> = String::from_utf8(output)
        .expect("UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("only JSON-RPC on stdout"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["id"], 1);
    assert!(lines[0]["result"]["capabilities"]["tools"].is_object());
    assert_eq!(lines[1]["id"], 2);
    let message = lines[1]["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("no running rBrowser instance for `/home/me/site`"),
        "{message}"
    );
}

/// Several instances and no project: the error lists them and the bridge goes on.
#[test]
fn several_instances_are_listed_in_the_error() {
    let line = json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/list" }).to_string();
    let mut output = Vec::new();
    let ended = forward(
        || Err(StdioError::Several(r"C:\a, C:\b".to_owned())),
        line.as_bytes(),
        &mut output,
    );
    assert!(matches!(ended, Err(StdioError::Several(_))));
    let answer: Value = serde_json::from_slice(&output).expect("one answer");
    let message = answer["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains(r"C:\a, C:\b"), "{message}");
    assert!(message.contains("--project"), "{message}");
}

/// An answer that cannot be read - larger than the limit, cut off, no JSON - still answers its
/// request: with a JSON-RPC error for its id, never silence (batch-3 security review,
/// finding 4).
#[test]
fn an_unreadable_answer_answers_its_request_with_an_error() {
    let id = json!(5);
    for body in [
        Err("the body is larger than 67108864 bytes".to_owned()),
        Ok(String::new()),
        Ok("{ not json".to_owned()),
    ] {
        let answers = answers_of(body.clone(), false, Some(&id));
        assert_eq!(answers.len(), 1, "{body:?}");
        assert_eq!(answers[0]["id"], 5);
        assert_eq!(answers[0]["jsonrpc"], "2.0");
        assert_eq!(answers[0]["error"]["code"], -32603);
        let message = answers[0]["error"]["message"].as_str().unwrap_or_default();
        assert!(message.contains("could not be read"), "{message}");
    }
    let too_large = answers_of(
        Err("larger than 67108864 bytes".to_owned()),
        false,
        Some(&id),
    );
    let message = too_large[0]["error"]["message"]
        .as_str()
        .unwrap_or_default();
    assert!(message.contains("67108864"), "{message}");
    assert!(
        answers_of(Err("cut".to_owned()), false, None).is_empty(),
        "a notification"
    );
}

#[test]
fn readable_answers_pass_as_they_are() {
    let id = json!(1);
    let one = json!({ "jsonrpc": "2.0", "id": 1, "result": {} });
    assert_eq!(
        answers_of(Ok(one.to_string()), false, Some(&id)),
        vec![one.clone()]
    );
    let stream = format!("event: message\ndata: {one}\n\n");
    assert_eq!(answers_of(Ok(stream), true, Some(&id)), vec![one]);
    assert_eq!(
        answers_of(Ok(String::new()), false, None),
        Vec::<Value>::new()
    );
}
