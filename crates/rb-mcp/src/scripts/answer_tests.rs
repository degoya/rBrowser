use super::*;
use crate::testing::answer_json;

const RAN: Ran = Ran {
    calls: 3,
    duration: Duration::from_millis(42),
    exceeded: false,
};

fn options() -> ScriptOptions {
    ScriptOptions {
        enabled: true,
        ..ScriptOptions::default()
    }
}

#[test]
fn a_value_comes_back_with_the_calls_the_time_and_the_console_lines() {
    let (is_error, body) = answer_json(&answer(
        Ok(r#"{"value":["mobile"],"logs":["checked 2 views"]}"#.to_owned()),
        RAN,
        &options(),
    ));
    assert!(!is_error);
    assert_eq!(
        body,
        json!({ "result": ["mobile"], "calls": 3, "durationMs": 42, "logs": ["checked 2 views"] })
    );
    let (_, quiet) = answer_json(&answer(
        Ok(r#"{"value":1,"logs":[]}"#.to_owned()),
        RAN,
        &options(),
    ));
    assert!(quiet.get("logs").is_none(), "{quiet}");
}

/// The answer's text as it is sent.
fn sent(result: &CallToolResult) -> String {
    let wire = serde_json::to_value(result).expect("serialises");
    wire["content"][0]["text"]
        .as_str()
        .expect("a text block")
        .to_owned()
}

fn capped_at(max_result_bytes: usize) -> ScriptOptions {
    ScriptOptions {
        max_result_bytes,
        ..options()
    }
}

#[test]
fn a_large_answer_is_cut_to_max_result_bytes_as_sent_with_a_note() {
    // Quotes and backslashes double when escaped: the cap holds for the text sent.
    let value = "\\\"".repeat(3000);
    let big = json!({ "value": value, "logs": ["x"] }).to_string();
    let result = answer(Ok(big), RAN, &capped_at(1024));
    let text = sent(&result);
    assert!(text.len() <= 1024, "{} bytes", text.len());
    let (is_error, body) = answer_json(&result);
    assert!(!is_error);
    assert_eq!(body["result"]["truncated"], true);
    assert!(
        body["result"]["preview"]
            .as_str()
            .is_some_and(|p| !p.is_empty())
    );
    let note = body["note"].as_str().expect("a note");
    assert!(
        note.contains("result was cut") && note.contains(MAX_RESULT_KEY),
        "{note}"
    );
    assert!(note.contains("console lines"), "{note}");
    assert!(body.get("logs").is_none());
}

#[test]
fn console_lines_over_the_cap_are_left_out_and_the_result_kept() {
    let logs: Vec<String> = (0..100)
        .map(|line| format!("line {line} of the log"))
        .collect();
    let answer_text = json!({ "value": ["mobile"], "logs": logs }).to_string();
    let result = answer(Ok(answer_text), RAN, &capped_at(1024));
    assert!(sent(&result).len() <= 1024);
    let (_, body) = answer_json(&result);
    assert_eq!(body["result"], json!(["mobile"]));
    assert!(body.get("logs").is_none());
    let note = body["note"].as_str().expect("a note");
    assert!(note.contains("console lines were left out"), "{note}");
    assert!(!note.contains("cut"), "{note}");
}

#[test]
fn a_failure_is_held_to_the_cap_too() {
    let failure = json!({ "code": "script.error", "name": "Error",
                          "message": "é".repeat(4000), "logs": ["y".repeat(4000)] })
    .to_string();
    let result = answer(Err(ScriptStop::Script(failure)), RAN, &capped_at(1024));
    assert!(sent(&result).len() <= 1024, "{} bytes", sent(&result).len());
    let (is_error, body) = answer_json(&result);
    assert!(is_error);
    assert_eq!(body["code"], "script.error");
    assert_eq!(body["truncated"], true);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|text| text.starts_with('é'))
    );
    assert!(body.get("logs").is_none());
}

#[test]
fn an_uncaught_tool_error_keeps_its_code_name_tool_and_line() {
    let failure = r#"{"code":"evaluate.disabled","name":"ToolError","message":"off","tool":"evaluate","line":2,"logs":[],"stack":"x"}"#;
    let (is_error, body) = answer_json(&answer(
        Err(ScriptStop::Script(failure.to_owned())),
        RAN,
        &options(),
    ));
    assert!(is_error);
    assert_eq!(
        body,
        json!({ "code": "evaluate.disabled", "error": "off", "name": "ToolError",
                "tool": "evaluate", "line": 2, "calls": 3, "durationMs": 42,
                "legacyCode": "evaluate_disabled" })
    );
}

#[test]
fn the_limits_answer_their_codes_and_settings() {
    let (_, timeout) = answer_json(&answer(Err(ScriptStop::Timeout), RAN, &options()));
    assert_eq!(timeout["code"], codes::TIMEOUT);
    assert_eq!(timeout["settingsKey"], TIMEOUT_KEY);
    let over = Ran {
        exceeded: true,
        ..RAN
    };
    // A script that caught the refusal and returned still answers the limit.
    let (_, limit) = answer_json(&answer(Ok(r#"{"value":1}"#.to_owned()), over, &options()));
    assert_eq!(limit["code"], codes::CALL_LIMIT);
    assert_eq!(limit["settingsKey"], MAX_CALLS_KEY);
    let (_, memory) = answer_json(&answer(Err(ScriptStop::OutOfMemory), RAN, &options()));
    assert_eq!(memory["code"], codes::MEMORY);
    let (_, syntax) = answer_json(&answer(
        Err(ScriptStop::Script(
            r#"{"code":"script.syntax","name":"SyntaxError","message":"bad","line":1}"#.to_owned(),
        )),
        RAN,
        &options(),
    ));
    assert_eq!(syntax["code"], codes::SYNTAX);
    assert_eq!(syntax["line"], 1);
}

#[test]
fn a_refusal_names_its_setting() {
    let (is_error, body) = answer_json(&refused(
        codes::DISABLED,
        "code mode is off",
        Some(super::super::ENABLED_KEY),
    ));
    assert!(is_error);
    assert_eq!(body["code"], "script.disabled");
    assert_eq!(body["settingsKey"], "mcp.scripts.enabled");
}
