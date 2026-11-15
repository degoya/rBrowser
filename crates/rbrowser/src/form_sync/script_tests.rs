use rb_sync::{AgentCommand, ChangeData, ReplayOutcome};
use serde_json::json;

use super::*;

fn apply(value: &str) -> AgentCommand {
    AgentCommand::Apply(ChangeData {
        value: Some(value.to_owned()),
        checked: None,
        selected: None,
        sensitive: true,
        target: serde_json::from_value(
            json!({ "element": { "kind": "css", "path": "#secret" }, "offset": { "x": 0.5, "y": 0.5 } }),
        )
        .expect("a target"),
    })
}

#[test]
fn the_expression_sets_the_marker_then_runs_the_command() {
    let source = expression(&apply("hunter2"), false).expect("an expression");
    let marker = source.find(r#""cmd":"replay""#).expect("the echo marker");
    let command = source.find(r#""cmd":"apply""#).expect("the command");
    assert!(marker < command);
    assert!(source.contains("applied:"));
    assert!(!source.contains("addEventListener('submit'"));
}

#[test]
fn the_submit_guard_comes_with_block_submit() {
    let source = expression(&apply("x"), true).expect("an expression");
    let guard = source.find("addEventListener('submit'").expect("the guard");
    assert!(guard < source.find(r#""cmd":"apply""#).expect("the command"));
}

#[test]
fn a_value_cannot_break_out_of_the_expression() {
    let source = expression(&apply("\"});alert(1);//</script>`${x}`"), false).expect("JSON");
    assert!(source.contains(r#"\"});alert(1);//"#));
}

#[test]
fn the_answer_becomes_the_outcome_without_quoting_it() {
    let cases = [
        (
            json!({ "applied": { "status": "done" } }),
            ReplayOutcome::Done,
        ),
        (
            json!({ "applied": { "status": "not_found" } }),
            ReplayOutcome::NotFound,
        ),
        (
            json!({ "applied": { "status": "not_visible" } }),
            ReplayOutcome::NotVisible,
        ),
    ];
    for (value, outcome) in cases {
        assert_eq!(outcome_of(&value), outcome);
    }
    let refused = outcome_of(&json!({ "applied": { "status": "error", "message": "hunter2" } }));
    let ReplayOutcome::Failed(reason) = refused else {
        panic!("not failed: {refused:?}");
    };
    assert!(!reason.contains("hunter2"));
    assert!(matches!(
        outcome_of(&json!({ "error": "no agent" })),
        ReplayOutcome::Failed(_)
    ));
    assert!(matches!(
        outcome_of(&json!({ "applied": "hunter2" })),
        ReplayOutcome::Failed(reason) if !reason.contains("hunter2")
    ));
}
