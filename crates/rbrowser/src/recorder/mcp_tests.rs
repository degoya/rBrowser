use rb_engine::ViewId;
use serde_json::json;

use super::*;
use crate::recorder::glue::Outcome;
use rb_sync::recorder::RecorderState;

#[test]
fn the_tools_are_named_as_the_commands_say() {
    assert_eq!(RECORDER_START_TOOL, "recorder_start");
    assert_eq!(RECORDER_STOP_TOOL, "recorder_stop");
    let defs = rb_commands::builtin::definitions();
    for (id, tool, chord) in [
        (
            "recorder.start",
            RECORDER_START_TOOL,
            "Ctrl+Shift+Space F S",
        ),
        ("recorder.stop", RECORDER_STOP_TOOL, "Ctrl+Shift+Space F T"),
        ("recorder.pause", RECORDER_STOP_TOOL, "Ctrl+Shift+Space F P"),
    ] {
        let def = defs
            .iter()
            .find(|def| def.id.as_str() == id)
            .unwrap_or_else(|| panic!("{id} is defined"));
        assert_eq!(
            def.mcp,
            rb_commands::McpCoverage::Covered(tool.into()),
            "{id}"
        );
        assert_eq!(def.default_bindings[0].to_string(), chord, "{id}");
        assert_eq!(def.group, rb_commands::Group::Flows, "{id}");
    }
}

#[test]
fn the_arguments_parse_with_their_defaults() {
    let args: RecorderStartArgs = parse(&serde_json::Value::Null).expect("parses");
    assert_eq!(args, RecorderStartArgs::default());
    let args: RecorderStartArgs =
        serde_json::from_value(json!({ "target": 2, "name": "login" })).expect("parses");
    assert_eq!(args.target, Some(Target::View(ViewId(2))));
    assert_eq!(args.name.as_deref(), Some("login"));
    let args: RecorderStopArgs =
        serde_json::from_value(json!({ "pause": true, "target": "all" })).expect("parses");
    assert!(args.pause);
    assert!(serde_json::from_value::<RecorderStopArgs>(json!({ "keep": true })).is_err());
}

#[test]
fn the_answer_carries_no_value_only_the_state() {
    let answer = RecorderAnswer::from(Outcome {
        state: RecorderState::Paused,
        name: Some("login".into()),
        steps: 3,
        path: None,
        not_captured: Vec::new(),
    });
    assert_eq!(
        serde_json::to_value(answer).expect("serialises"),
        json!({ "state": "paused", "name": "login", "steps": 3 })
    );
}

#[test]
fn the_descriptions_name_the_secrets_and_the_no_overwrite_rule() {
    assert!(RECORDER_START_DESCRIPTION.contains("{{secret:<name>}}"));
    assert!(RECORDER_STOP_DESCRIPTION.contains("never overwritten"));
}

#[test]
fn the_answer_names_the_kinds_not_captured() {
    let answer = RecorderAnswer::from(Outcome {
        state: RecorderState::Recording,
        name: Some("login".into()),
        steps: 0,
        path: None,
        not_captured: vec!["keyboard"],
    });
    assert_eq!(
        serde_json::to_value(answer).expect("serialises")["notCaptured"],
        json!(["keyboard"])
    );
}
