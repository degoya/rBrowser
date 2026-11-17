use rb_plugin_api::FailureKind;
use serde_json::json;

use super::*;
use crate::fake_host::{FakeHost, view};

fn host() -> FakeHost {
    FakeHost::with_views(vec![
        view(1, "Mobile", 375),
        view(2, "Tablet", 768),
        view(3, "Desktop", 1280),
    ])
}

fn outcome(ok: bool, text: &str) -> Result<String, Failure> {
    Ok(json!({ "ok": ok, "type": "string", "text": text }).to_string())
}

#[test]
fn equal_results_of_several_views_are_one_line() {
    let mut host = host();
    host.answers.insert(1, outcome(true, "\"Shop\""));
    host.answers.insert(2, outcome(true, "\"Shop\""));
    host.answers.insert(3, outcome(true, "\"Shop - Desktop\""));
    let answer = run(&mut host, "document.title", Target::All).expect("runs");
    assert_eq!(
        answer["results"],
        json!([
            { "ok": true, "type": "string", "text": "\"Shop\"", "views": [1, 2] },
            { "ok": true, "type": "string", "text": "\"Shop - Desktop\"", "views": [3] },
        ])
    );
    assert_eq!(answer["expression"], "document.title");
    assert_eq!(host.evaluated.len(), 3);
}

#[test]
fn a_throw_and_a_failing_view_are_results_too() {
    let mut host = host();
    host.answers
        .insert(1, outcome(false, "ReferenceError: nope is not defined"));
    host.answers.insert(
        2,
        Err(Failure::coded(
            FailureKind::NotSupported,
            "plugins.error.not_available",
        )),
    );
    let answer = run(&mut host, "nope", Target::All).expect("runs");
    let results = answer["results"].as_array().expect("a list");
    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["ok"], false);
    assert_eq!(results[1]["text"], "plugins.error.not_available");
    assert_eq!(results[2]["text"], "null");
}

#[test]
fn the_active_view_or_one_view_only() {
    let mut host = host();
    host.active = Some(2);
    run(&mut host, "1 + 1", Target::Active).expect("runs");
    assert_eq!(host.evaluated.iter().map(|e| e.0).collect::<Vec<_>>(), [2]);

    let mut host = self::host();
    let refused = run(&mut host, "1", Target::Active).expect_err("no active view");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.console.errors.no_active_view")
    );

    let mut host = self::host();
    run(&mut host, "1", Target::View(3)).expect("runs");
    assert_eq!(host.evaluated.iter().map(|e| e.0).collect::<Vec<_>>(), [3]);
    let refused = run(&mut host, "1", Target::View(9)).expect_err("no such view");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.console.errors.unknown_view")
    );
}

#[test]
fn empty_and_overlong_expressions_are_refused() {
    let mut host = host();
    let refused = run(&mut host, "  ", Target::All).expect_err("empty");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.console.errors.no_expression")
    );
    let long = "1".repeat(MAX_EXPRESSION_CHARS + 1);
    let refused = run(&mut host, &long, Target::All).expect_err("too long");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.console.errors.expression_too_long")
    );
    assert!(host.evaluated.is_empty());
}

#[test]
fn the_script_wraps_the_expression_on_lines_of_its_own() {
    let wrapped = script("document.title; ");
    assert!(wrapped.contains("\ndocument.title\n"));
    assert!(!wrapped.contains("__EXPRESSION__"));
    // A line comment at the end cannot swallow the wrapper's closing parenthesis.
    assert!(script("1 // note").contains("1 // note\n    );"));
}

#[test]
fn targets_read_from_arguments() {
    assert_eq!(
        Target::parse(&json!(null), Target::Active).ok(),
        Some(Target::Active)
    );
    assert_eq!(
        Target::parse(&json!("all"), Target::Active).ok(),
        Some(Target::All)
    );
    assert_eq!(
        Target::parse(&json!(4), Target::All).ok(),
        Some(Target::View(4))
    );
    assert!(Target::parse(&json!("some"), Target::All).is_err());
    assert!(Target::parse(&json!(-1), Target::All).is_err());
}
