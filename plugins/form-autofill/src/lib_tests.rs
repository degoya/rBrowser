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
fn the_preset_tools_and_command_answer() {
    // RB-090-70: capture, save and list - the command only asks the panel to capture.
    let mut host = host();
    host.collect(
        1,
        json!({ "fields": [{ "index": 0, "tag": "input", "type": "email", "name": "email",
                             "value": "qa@example.com" }] }),
    );
    let asked: Value = serde_json::from_str(
        &FormAutofill
            .run_command(&mut host, "capture", "{}")
            .expect("asks"),
    )
    .expect("json");
    assert_eq!(asked["$open"]["panel"], "autofill");
    let captured: Value = serde_json::from_str(
        &FormAutofill
            .call_tool(&mut host, "capture", r#"{"target":1}"#)
            .expect("captures"),
    )
    .expect("json");
    assert_eq!(captured["fields"][0]["value"], "qa@example.com");
    let saved: Value = serde_json::from_str(
        &FormAutofill
            .call_tool(
                &mut host,
                "save",
                r#"{"name":"qa","values":{"email":"qa@example.com"}}"#,
            )
            .expect("saves"),
    )
    .expect("json");
    assert_eq!(saved["saved"], "qa");
    let listed: Value = serde_json::from_str(
        &FormAutofill
            .call_tool(&mut host, "presets", "{}")
            .expect("lists"),
    )
    .expect("json");
    assert_eq!(listed["presets"][0]["name"], "qa");
    let failure = FormAutofill
        .call_tool(&mut host, "save", r#"{"name":"qa"}"#)
        .expect_err("no values");
    assert_eq!(failure.code.as_deref(), Some(codes::INVALID));
}

#[test]
fn every_manifest_tool_and_command_is_served() {
    let manifest = include_str!("../manifest.toml");
    for tool in ["fill", "capture", "save", "presets", "rename", "delete"] {
        assert!(manifest.contains(&format!("name = \"{tool}\"")), "{tool}");
    }
    for command in ["fill_valid", "fill_invalid", "capture"] {
        assert!(
            manifest.contains(&format!("id = \"{command}\"")),
            "{command}"
        );
    }
}

#[test]
fn keys_live_below_the_plugin() {
    assert_eq!(
        key("errors.profile_unknown"),
        "plugin.form-autofill.errors.profile_unknown"
    );
}

/// RB-090-71: a stored preset renamed and deleted through the tools the panel and MCP call, and
/// a preset filling from the fill command through `activeProfile`.
#[test]
fn a_preset_is_renamed_deleted_and_filled_through_the_tools() {
    let mut host = host();
    let call = |host: &mut FakeHost, tool: &str, args: &str| {
        FormAutofill
            .call_tool(host, tool, args)
            .map(|answer| serde_json::from_str::<Value>(&answer).expect("json"))
    };
    call(
        &mut host,
        "save",
        r#"{"name":"qa","values":{"email":"qa@example.com"}}"#,
    )
    .expect("saves");
    let renamed = call(&mut host, "rename", r#"{"name":"qa","to":"Checkout"}"#).expect("renames");
    assert_eq!(renamed["to"], "Checkout");
    let listed = call(&mut host, "presets", "{}").expect("lists");
    assert_eq!(listed["presets"][0]["name"], "Checkout");
    // The fill command takes a preset the settings name, as the panel's choice passes it.
    host.set("activeProfile", "\"Checkout\"");
    let filled: Value = serde_json::from_str(
        &FormAutofill
            .run_command(&mut host, "fill_valid", "{}")
            .expect("fills"),
    )
    .expect("json");
    assert_eq!(filled["profile"], "Checkout");
    let deleted = call(&mut host, "delete", r#"{"name":"Checkout"}"#).expect("deletes");
    assert_eq!(deleted["deleted"], "Checkout");
    let listed = call(&mut host, "presets", "{}").expect("lists");
    assert_eq!(listed["presets"], serde_json::json!([]));
    let failure = call(&mut host, "delete", r#"{"name":"Checkout"}"#).expect_err("gone");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.form-autofill.errors.preset_unknown")
    );
    let failure = call(&mut host, "rename", r#"{"to":"x"}"#).expect_err("no name");
    assert_eq!(failure.code.as_deref(), Some(codes::INVALID));
}
