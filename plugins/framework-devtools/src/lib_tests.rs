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

#[test]
fn the_commands_open_their_panel_and_nothing_else_is_a_command() {
    let mut host = host();
    for framework in FRAMEWORKS {
        let text = FrameworkDevtools
            .run_command(&mut host, framework, "")
            .expect("answers");
        assert_eq!(answer(&text), json!({ "$open": { "panel": framework } }));
    }
    let error = FrameworkDevtools
        .run_command(&mut host, "svelte", "{}")
        .expect_err("unknown");
    assert_eq!(error.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
}

#[test]
fn active_view_names_the_view_or_null() {
    let mut host = host();
    let text = FrameworkDevtools
        .call_tool(&mut host, ACTIVE_VIEW, "{}")
        .expect("answers");
    assert_eq!(answer(&text), json!({ "view": null }));
    host.active = Some(3);
    let text = FrameworkDevtools
        .call_tool(&mut host, ACTIVE_VIEW, "")
        .expect("answers");
    assert_eq!(answer(&text), json!({ "view": 3 }));
}

#[test]
fn send_posts_the_envelope_to_the_view() {
    let mut host = host();
    let text = FrameworkDevtools
        .call_tool(
            &mut host,
            SEND,
            r#"{"view":2,"framework":"react","envelope":{"t":"m","d":{"event":"inspectElement","payload":{"id":5}}}}"#,
        )
        .expect("answers");
    assert_eq!(
        answer(&text),
        json!({ "$post": [{ "target": 2, "message": {
            "framework": "react",
            "envelope": { "t": "m", "d": { "event": "inspectElement", "payload": { "id": 5 } } },
        } }] })
    );
    let text = FrameworkDevtools
        .call_tool(
            &mut host,
            SEND,
            r#"{"view":0,"framework":"vue","envelope":{"t":"connect"}}"#,
        )
        .expect("answers");
    assert_eq!(
        answer(&text)["$post"][0]["message"]["envelope"],
        json!({ "t": "connect" })
    );
}

#[test]
fn send_refuses_what_is_not_an_envelope_for_a_known_framework_and_view() {
    let mut host = host();
    for (args, reason) in [
        (r#"{"framework":"vue","envelope":{"t":"connect"}}"#, "view"),
        (
            r#"{"view":-1,"framework":"vue","envelope":{"t":"connect"}}"#,
            "view",
        ),
        (
            r#"{"view":1,"framework":"svelte","envelope":{"t":"connect"}}"#,
            "framework",
        ),
        (
            r#"{"view":1,"framework":"vue","envelope":{"t":"other"}}"#,
            "envelope",
        ),
        (
            r#"{"view":1,"framework":"vue","envelope":{"t":"m"}}"#,
            "envelope",
        ),
        (
            r#"{"view":1,"framework":"vue","envelope":"connect"}"#,
            "envelope",
        ),
        ("[]", "args"),
    ] {
        let error = FrameworkDevtools
            .call_tool(&mut host, SEND, args)
            .expect_err(args);
        assert_eq!(error.code.as_deref(), Some(codes::INVALID), "{args}");
        assert_eq!(
            error.params.get("reason").map(String::as_str),
            Some(reason),
            "{args}"
        );
    }
}

#[test]
fn send_refuses_an_envelope_over_the_message_limit() {
    let mut host = host();
    let big = "x".repeat(MAX_ENVELOPE_BYTES);
    let args = format!(r#"{{"view":1,"framework":"vue","envelope":{{"t":"m","d":"{big}"}}}}"#);
    let error = FrameworkDevtools
        .call_tool(&mut host, SEND, &args)
        .expect_err("too large");
    assert_eq!(error.kind, FailureKind::Limit);
    assert_eq!(error.code.as_deref(), Some(codes::TOO_LARGE));
}

#[test]
fn an_unknown_tool_is_refused() {
    let mut host = host();
    let error = FrameworkDevtools
        .call_tool(&mut host, "tree", "{}")
        .expect_err("unknown");
    assert_eq!(error.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
}
