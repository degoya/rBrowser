use super::*;
use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;

use crate::store::{STATE_FILE, STATE_PATH};

fn host() -> MemoryHost {
    MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
}

fn command(host: &mut MemoryHost, args: &str) -> Value {
    let answer = ImageOverlay
        .run_command(host, "toggle", args)
        .expect("toggle");
    if let Some(bytes) = host.out.get(STATE_FILE).cloned() {
        host.project.insert(STATE_PATH.to_owned(), bytes);
    }
    serde_json::from_str(&answer).expect("json")
}

#[test]
fn toggle_switches_the_overlay_in_every_view_and_on_sets_it() {
    let mut host = host();
    let on = command(&mut host, "");
    assert_eq!(on["state"]["enabled"], json!(true));
    assert_eq!(on["$post"][0]["target"], json!("all"));
    assert_eq!(on["$post"][0]["message"]["reload"], json!(true));
    let off = command(&mut host, "{}");
    assert_eq!(off["state"]["enabled"], json!(false));
    assert_eq!(off["$post"][0]["message"]["reload"], json!(false));
    let still_on = command(&mut host, r#"{"on": true}"#);
    let again = command(&mut host, r#"{"on": true}"#);
    assert_eq!(still_on["state"]["enabled"], json!(true));
    assert_eq!(again["state"]["enabled"], json!(true));
}

#[test]
fn a_change_without_a_project_is_kept_for_the_session_and_said_so() {
    let mut host = MemoryHost::new();
    let answer = ImageOverlay
        .run_command(&mut host, "toggle", "")
        .expect("toggle");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(answer["saved"], json!(false));
    assert_eq!(
        answer["$post"][0]["message"]["overrides"]["enabled"],
        json!(true)
    );
}

#[test]
fn unknown_contributions_are_refused() {
    let mut host = host();
    let command = ImageOverlay
        .run_command(&mut host, "explode", "")
        .expect_err("unknown command");
    assert_eq!(
        command.code.as_deref(),
        Some("plugins.error.unknown_contribution")
    );
    let tool = ImageOverlay
        .call_tool(&mut host, "get", "{}")
        .expect_err("unknown tool");
    assert_eq!(
        tool.code.as_deref(),
        Some("plugins.error.unknown_contribution")
    );
    let bad = ImageOverlay
        .run_command(&mut host, "toggle", "[")
        .expect_err("bad args");
    assert_eq!(
        bad.code.as_deref(),
        Some("plugin.image-overlay.errors.bad_args")
    );
}

#[test]
fn a_full_chunk_fits_the_page_message_limit() {
    // base64 of CHUNK_BYTES plus the answer's envelope stays below 64 KiB.
    assert!(CHUNK_BYTES.div_ceil(3) * 4 + 64 < 64 * 1024);
}

#[test]
fn add_design_asks_the_application_for_the_dialog_and_says_so() {
    let mut host = host();
    let answer = ImageOverlay
        .run_command(&mut host, "add_design", "{}")
        .expect("add_design");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(answer["started"], json!(true));
    let posts = answer["$post"].as_array().expect("posts");
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0]["target"], json!("status"));
    assert_eq!(
        posts[0]["message"],
        json!({ "key": "plugin.image-overlay.status.choose_design", "level": "info" })
    );
    // The status first: the application's own outcome then replaces it.
    assert_eq!(
        posts[1],
        json!({ "target": "host", "message": { "action": "add_design" } })
    );
    // Nothing is written by the plugin itself.
    assert!(host.out.is_empty());
}

#[test]
fn the_designs_tool_is_served() {
    let mut host = host();
    let answer = ImageOverlay
        .call_tool(&mut host, DESIGNS_TOOL, "")
        .expect("designs");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(answer["designs"], json!([]));
}
