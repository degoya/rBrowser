use rb_sync::{CommandResult, ElementTarget};
use serde_json::json;

use super::*;

fn target(path: &str) -> ElementTarget {
    serde_json::from_value(json!({
        "element": { "kind": "css", "path": path },
        "offset": { "x": 0.5, "y": 0.5 },
    }))
    .expect("a target")
}

#[test]
fn the_echo_marker_comes_before_the_resolution() {
    let source = expression(Some(&target("#save")), false).expect("an expression");
    let marker = source
        .find(r#"agent.command({"cmd":"replay","ms":300})"#)
        .expect("the marker");
    let resolve = source.find(r#""cmd":"resolve""#).expect("the resolve");
    assert!(marker < resolve);
    assert!(source.contains("#save"));
    assert!(source.contains("globalThis.__rbAgent"));
}

#[test]
fn keys_set_the_marker_and_resolve_nothing() {
    let source = expression(None, false).expect("an expression");
    assert!(source.contains(r#""cmd":"replay""#));
    assert!(!source.contains(r#""cmd":"resolve""#));
    assert!(source.contains("const resolved=null"));
}

#[test]
fn the_submit_guard_is_there_only_with_block_submit() {
    let free = expression(Some(&target("#go")), false).expect("an expression");
    assert!(!free.contains("submit"));
    let blocked = expression(Some(&target("#go")), true).expect("an expression");
    assert!(blocked.contains("addEventListener('submit',block,true)"));
    assert!(blocked.contains("preventDefault()"));
    assert!(blocked.contains("stopImmediatePropagation()"));
    assert!(blocked.contains(&format!("Date.now()+{BLOCK_SUBMIT_MS}")));
    // The guard is set before the input, so it is in place when the click lands.
    let guard = blocked.find("addEventListener").expect("the guard");
    assert!(guard < blocked.find(r#""cmd":"resolve""#).expect("the resolve"));
}

#[test]
fn a_hostile_selector_stays_inside_its_json_string() {
    let source = expression(Some(&target(r#"a"]'),alert(1),('"#)), true).expect("an expression");
    assert!(source.contains(r#""path":"a\"]'),alert(1),('""#));
}

#[test]
fn the_answer_is_read_with_its_viewport() {
    let answer = parse(&json!({
        "resolved": { "status": "ok", "x": 10.5, "y": 20.0, "scrolled": true },
        "viewport": [0.0, 120.0, 1.5],
    }))
    .expect("an answer");
    assert_eq!(
        answer.resolved,
        Some(CommandResult::Ok {
            x: 10.5,
            y: 20.0,
            scrolled: true
        })
    );
    assert_eq!(
        answer.viewport,
        Some(VisualViewport {
            offset_left: 0.0,
            offset_top: 120.0,
            scale: 1.5
        })
    );
}

#[test]
fn misses_and_missing_parts_are_read() {
    let answer = parse(&json!({ "resolved": { "status": "not_found" }, "viewport": null }))
        .expect("an answer");
    assert_eq!(answer.resolved, Some(CommandResult::NotFound));
    assert_eq!(answer.viewport, None);
    let keys = parse(&json!({ "resolved": null, "viewport": ["x", 1, 1] })).expect("an answer");
    assert_eq!(
        keys,
        Answer {
            resolved: None,
            viewport: None
        }
    );
}

#[test]
fn no_agent_and_garbage_are_errors_without_the_value() {
    assert!(parse(&json!({ "error": "no agent" })).is_err());
    let error = parse(&json!({ "resolved": { "status": "s3cret" } })).expect_err("garbage");
    assert!(!error.contains("s3cret"));
}
