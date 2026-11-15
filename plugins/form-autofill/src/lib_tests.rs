use super::*;
use crate::fake::FakeHost;
use serde_json::{Value, json};

fn host() -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Mobile", "chromium").collect(
        1,
        json!({ "locale": "en-US", "fields": [
            { "index": 0, "key": "input:email", "sig": "s", "tag": "input", "type": "email",
              "name": "email", "required": true } ] }),
    );
    host
}

#[test]
fn the_commands_fill_valid_and_invalid_and_post_to_the_panel() {
    let mut host = host();
    let answer: Value = serde_json::from_str(
        &FormAutofill
            .run_command(&mut host, "fill_valid", "{}")
            .expect("runs"),
    )
    .expect("json");
    assert_eq!(answer["mode"], "valid");
    assert_eq!(answer["$post"][0]["target"], "panel");
    let answer: Value = serde_json::from_str(
        &FormAutofill
            .run_command(&mut host, "fill_invalid", "{}")
            .expect("runs"),
    )
    .expect("json");
    assert_eq!(answer["mode"], "invalid");
}

#[test]
fn the_tool_fills_by_its_mode_and_posts_too() {
    let mut host = host();
    let answer: Value = serde_json::from_str(
        &FormAutofill
            .call_tool(&mut host, "fill", r#"{"mode":"invalid","target":"all"}"#)
            .expect("runs"),
    )
    .expect("json");
    assert_eq!(answer["mode"], "invalid");
    assert_eq!(answer["views"][0]["fields"][0]["kind"], "email");
    assert!(answer.get("$post").is_some());
}

#[test]
fn unknown_names_and_bad_arguments_fail() {
    let mut host = host();
    let failure = FormAutofill
        .run_command(&mut host, "explode", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = FormAutofill
        .call_tool(&mut host, "@page", "{}")
        .expect_err("no page scripts");
    assert_eq!(failure.code.as_deref(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = FormAutofill
        .call_tool(&mut host, "fill", r#"{"mode":"loud"}"#)
        .expect_err("bad mode");
    assert_eq!(failure.code.as_deref(), Some(codes::INVALID));
}

#[test]
fn keys_live_below_the_plugin() {
    assert_eq!(
        key("errors.profile_unknown"),
        "plugin.form-autofill.errors.profile_unknown"
    );
}
