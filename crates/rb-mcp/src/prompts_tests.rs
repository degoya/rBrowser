use std::sync::Arc;

use rmcp::model::JsonObject;
use serde_json::{Value, json};

use super::{DISABLED, INVALID, NAMES, NOT_FOUND, SETTING, get, list};
use crate::RbMcpServer;
use crate::backend::McpOptions;
use crate::testing::{FakeBackend, server};

fn args(value: Value) -> JsonObject {
    value.as_object().cloned().expect("an object")
}

/// The text of the prompt's one message.
fn text(name: &str, arguments: Option<Value>) -> String {
    let arguments = arguments.map(args);
    let result = get(name, arguments.as_ref()).expect("the prompt");
    let wire = serde_json::to_value(&result).expect("JSON");
    assert_eq!(wire["messages"].as_array().map(Vec::len), Some(1), "{wire}");
    assert_eq!(wire["messages"][0]["role"], "user");
    wire["messages"][0]["content"]["text"]
        .as_str()
        .expect("a text message")
        .to_owned()
}

#[test]
fn the_three_prompts_are_listed_with_url_and_breakpoints() {
    let prompts = serde_json::to_value(list()).expect("JSON");
    let names: Vec<&str> = prompts
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|prompt| prompt["name"].as_str())
        .collect();
    assert_eq!(names, ["responsive_review", "a11y_triage", "bug_repro"]);
    assert_eq!(names, NAMES);
    for prompt in prompts.as_array().expect("a list") {
        let arguments: Vec<&str> = prompt["arguments"]
            .as_array()
            .expect("arguments")
            .iter()
            .filter_map(|argument| argument["name"].as_str())
            .collect();
        assert_eq!(arguments, ["url", "breakpoints"], "{prompt}");
        for argument in prompt["arguments"].as_array().expect("arguments") {
            assert_eq!(argument["required"], false, "{argument}");
            let description = argument["description"].as_str().expect("described");
            assert!(!description.starts_with("mcp.prompt."), "{description}");
        }
        for field in ["title", "description"] {
            let text = prompt[field].as_str().expect(field);
            assert!(!text.starts_with("mcp.prompt."), "the catalogue has {text}");
        }
    }
}

#[test]
fn responsive_review_carries_the_url_and_the_breakpoints() {
    let with = text(
        "responsive_review",
        Some(json!({ "url": "https://shop.test/cart?x=1", "breakpoints": "mobile, desktop" })),
    );
    assert!(with.contains("https://shop.test/cart?x=1"), "{with}");
    assert!(with.contains("mobile, desktop"), "{with}");
    let without = text("responsive_review", None);
    assert!(!without.contains("https://"), "{without}");
    assert!(without.contains("views_list"), "{without}");
    assert_ne!(with, without);
}

#[test]
fn an_empty_argument_counts_as_absent() {
    assert_eq!(
        text(
            "a11y_triage",
            Some(json!({ "url": "  ", "breakpoints": "" }))
        ),
        text("a11y_triage", None)
    );
}

#[test]
fn every_text_is_filled_and_names_only_real_tools() {
    let tools = RbMcpServer::tool_names();
    for name in NAMES {
        let text = text(name, Some(json!({ "url": "https://a.test/" })));
        assert!(!text.starts_with("mcp.prompt."), "the catalogue has {name}");
        assert!(!text.contains('{') && !text.contains('}'), "{name}: {text}");
        let words: Vec<&str> = text
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .collect();
        let tools_named = words
            .iter()
            .filter(|word| tools.iter().any(|tool| tool == *word))
            .count();
        assert!(tools_named >= 3, "{name} names {tools_named} tools: {text}");
        // Every snake_case word is a tool's name (`views_list`, not a made-up one).
        let snake = words.into_iter().filter(|word| {
            word.contains('_') && word.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        });
        for word in snake {
            assert!(
                tools.iter().any(|tool| tool == word),
                "{name}: `{word}` is no tool"
            );
        }
    }
    assert!(text("a11y_triage", None).contains("checks_run"));
    assert!(text("bug_repro", None).contains("bug_report"));
}

#[test]
fn unknown_prompts_and_arguments_are_refused() {
    let code =
        |name: &str, arguments: Value| get(name, Some(&args(arguments))).expect_err("refused").code;
    assert_eq!(code("nope", json!({})), NOT_FOUND);
    assert_eq!(code("responsive_review", json!({ "steps": "1" })), INVALID);
    assert_eq!(code("responsive_review", json!({ "url": 42 })), INVALID);
    assert_eq!(
        code("responsive_review", json!({ "url": "https://a.test/ b" })),
        INVALID
    );
    assert_eq!(
        code(
            "responsive_review",
            json!({ "url": "https://a.test/\u{7}" })
        ),
        INVALID
    );
    let long = format!("https://a.test/{}", "a".repeat(super::MAX_URL_CHARS));
    assert_eq!(code("responsive_review", json!({ "url": long })), INVALID);
    assert_eq!(
        code("bug_repro", json!({ "breakpoints": "mobile,<b>" })),
        INVALID
    );
    assert_eq!(
        code("bug_repro", json!({ "breakpoints": "mobile,,desktop" })),
        INVALID
    );
    let many: Vec<String> = (0..=super::MAX_BREAKPOINTS)
        .map(|n| format!("bp{n}"))
        .collect();
    let many = many.join(",");
    assert_eq!(code("bug_repro", json!({ "breakpoints": many })), INVALID);
}

#[test]
fn a_breakpoint_named_twice_is_named_once() {
    let text = text(
        "bug_repro",
        Some(json!({ "breakpoints": "mobile,desktop,mobile" })),
    );
    assert!(text.contains("mobile, desktop"), "{text}");
    assert!(!text.contains("mobile, desktop, mobile"), "{text}");
}

#[test]
fn with_mcp_prompts_off_none_is_listed_or_answered() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    assert_eq!(server.prompt_list().len(), 3);
    assert!(server.prompt_get("bug_repro", None).is_ok());
    fake.set_options(McpOptions {
        prompts: false,
        ..McpOptions::default()
    });
    assert!(server.prompt_list().is_empty());
    let refused = server
        .prompt_get("bug_repro", None)
        .expect_err("prompts are off");
    assert_eq!(refused.code, DISABLED);
    assert_eq!(refused.settings_key.as_deref(), Some(SETTING));
}
