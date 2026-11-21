use rb_plugin_api::feature::Feature;
use serde_json::json;

use super::LiveCss;
use crate::testing::{host, json};

#[test]
fn the_panel_command_opens_the_editor() {
    let answer = LiveCss
        .run_command(&mut host(), "panel", "")
        .expect("opens");
    assert_eq!(json(&answer), json!({ "$open": { "panel": "editor" } }));
}

#[test]
fn the_toggle_command_switches_the_draft() {
    let mut host = host();
    LiveCss
        .call_tool(&mut host, "set", r#"{"css": "a{}"}"#)
        .expect("set");
    let answer = json(
        &LiveCss
            .run_command(&mut host, "toggle", "{}")
            .expect("toggled"),
    );
    assert_eq!(answer["enabled"], json!(false));
}

#[test]
fn the_tools_and_the_page_request_answer_json() {
    let mut host = host();
    let set = json(
        &LiveCss
            .call_tool(&mut host, "set", r#"{"css": "a{}", "breakpoints": ["lg"]}"#)
            .expect("set"),
    );
    assert_eq!(set["blocks"][0]["media"], json!("(min-width: 1280px)"));
    let page = json(
        &LiveCss
            .call_tool(
                &mut host,
                "@page",
                r#"{"view": 3, "message": {"op": "sheet", "reason": "start"}}"#,
            )
            .expect("answers"),
    );
    assert_eq!(page["css"], json!("@media (min-width: 1280px) {\n  a{}\n}"));
    let cleared = json(&LiveCss.call_tool(&mut host, "clear", "").expect("cleared"));
    assert_eq!(cleared["blocks"], json!([]));
}

#[test]
fn unknown_contributions_and_arguments_that_are_no_object_are_refused() {
    let unknown = LiveCss
        .call_tool(&mut host(), "nope", "{}")
        .expect_err("unknown");
    assert_eq!(
        unknown.code.as_deref(),
        Some("plugins.error.unknown_contribution")
    );
    let command = LiveCss
        .run_command(&mut host(), "nope", "")
        .expect_err("unknown");
    assert_eq!(
        command.code.as_deref(),
        Some("plugins.error.unknown_contribution")
    );
    let bad = LiveCss
        .call_tool(&mut host(), "set", "[1]")
        .expect_err("bad");
    assert_eq!(bad.code.as_deref(), Some("plugin.live-css.errors.bad_args"));
}
