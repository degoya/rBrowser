use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, FailureKind, codes};
use serde_json::{Value, json};

use super::*;

fn host() -> MemoryHost {
    MemoryHost::new().grant(Capability::Views)
}

fn answer(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON")
}

fn posted_to(target: Value) -> Value {
    json!({ "$post": [{ "target": target, "message": { "type": "toggleRaw" } }] })
}

#[test]
fn toggle_raw_posts_to_the_active_view() {
    let mut host = host();
    host.active = Some(3);
    let text = Viewer
        .run_command(&mut host, TOGGLE_RAW, "{}")
        .expect("answers");
    assert_eq!(answer(&text), posted_to(json!(3)));
}

#[test]
fn toggle_raw_posts_to_every_view_without_an_active_one_or_when_asked() {
    let mut host = host();
    let text = Viewer
        .run_command(&mut host, TOGGLE_RAW, "")
        .expect("answers");
    assert_eq!(answer(&text), posted_to(json!("all")));
    host.active = Some(3);
    let text = Viewer
        .run_command(&mut host, TOGGLE_RAW, r#"{"target":"all"}"#)
        .expect("answers");
    assert_eq!(answer(&text), posted_to(json!("all")));
    let text = Viewer
        .run_command(&mut host, TOGGLE_RAW, r#"{"target":5}"#)
        .expect("answers");
    assert_eq!(answer(&text), posted_to(json!(5)));
}

#[test]
fn toggle_raw_refuses_a_malformed_target() {
    let mut host = host();
    for args in [
        r#"{"target":"some"}"#,
        r#"{"target":-1}"#,
        r#"{"target":4294967296}"#,
        "[1]",
        "{",
    ] {
        let failure = Viewer
            .run_command(&mut host, TOGGLE_RAW, args)
            .expect_err(args);
        assert_eq!(failure.kind, FailureKind::BadData, "{args}");
        assert_eq!(failure.code.as_deref(), Some(codes::INVALID), "{args}");
    }
}

#[test]
fn without_the_views_grant_the_command_posts_to_every_view() {
    let mut host = MemoryHost::new();
    host.active = Some(3);
    let text = Viewer
        .run_command(&mut host, TOGGLE_RAW, "{}")
        .expect("answers");
    assert_eq!(answer(&text), posted_to(json!("all")));
}

#[test]
fn the_page_gets_the_settings() {
    let mut host = host();
    host.settings
        .insert("theme".to_owned(), "\"dark\"".to_owned());
    let text = Viewer
        .call_tool(
            &mut host,
            PAGE_TOOL,
            &json!({ "view": 1, "message": { "type": "config" } }).to_string(),
        )
        .expect("answers");
    assert_eq!(answer(&text)["config"]["theme"], json!("dark"));
    assert_eq!(answer(&text)["config"]["maxBytes"], json!(20_971_520));
}

#[test]
fn the_page_and_the_command_are_served_and_nothing_else() {
    let mut host = host();
    let failure = Viewer
        .call_tool(
            &mut host,
            PAGE_TOOL,
            r#"{"view":1,"message":{"type":"eval"}}"#,
        )
        .expect_err("unknown message");
    assert_eq!(failure.code.as_deref(), Some(codes::INVALID));
    let failure = Viewer
        .call_tool(&mut host, "toggle_raw", "{}")
        .expect_err("no tools");
    assert_eq!(failure.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = Viewer
        .run_command(&mut host, "launch", "{}")
        .expect_err("unknown command");
    assert_eq!(failure.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
}
