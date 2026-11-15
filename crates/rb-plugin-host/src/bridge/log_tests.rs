use serde_json::json;

use super::*;

#[test]
fn every_console_level_has_its_name() {
    for (name, level) in [
        ("error", LogLevel::Error),
        ("warning", LogLevel::Warning),
        ("info", LogLevel::Info),
        ("log", LogLevel::Log),
        ("debug", LogLevel::Debug),
    ] {
        assert_eq!(LogLevel::parse(name), Some(level));
        assert_eq!(level.as_str(), name);
    }
    assert_eq!(LogLevel::parse("warn"), None);
}

#[test]
fn a_log_keeps_at_most_its_number_of_texts_each_cut() {
    let values: Vec<String> = (0..MAX_LOG_VALUES + 3).map(|n| n.to_string()).collect();
    let log = PageLog::from_value(json!({ "level": "log", "values": values })).expect("a log");
    assert_eq!(log.values.len(), MAX_LOG_VALUES);
    let long = "\u{e9}".repeat(MAX_LOG_CHARS + 10);
    let log = PageLog::from_value(json!({ "level": "log", "values": [long] })).expect("a log");
    assert_eq!(log.values[0].chars().count(), MAX_LOG_CHARS);
}

#[test]
fn anything_but_a_level_and_texts_is_no_log() {
    assert!(PageLog::from_value(json!([])).is_none());
    assert!(PageLog::from_value(json!({ "level": "log" })).is_none());
    assert!(PageLog::from_value(json!({ "level": "log", "values": "x" })).is_none());
    assert!(PageLog::from_value(json!({ "level": "log", "values": [null] })).is_none());
}

// Review of RB-080-66, 4: a page script's throw keeps its stack.
#[test]
fn an_error_line_keeps_its_stack_cut() {
    let log = PageLog::from_value(json!({
        "level": "error",
        "values": ["[demo]", "boom"],
        "stack": "Error: boom\n    at run".repeat(1000),
    }))
    .expect("a log");
    let stack = log.stack.expect("a stack");
    assert!(stack.starts_with("Error: boom"));
    assert_eq!(stack.chars().count(), MAX_STACK_CHARS);
    assert!(PageLog::from_value(json!({ "level": "log", "values": [], "stack": 3 })).is_none());
    let plain = PageLog::from_value(json!({ "level": "log", "values": [] })).expect("a log");
    assert_eq!(plain.stack, None);
}
