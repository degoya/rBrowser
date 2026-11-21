use serde_json::json;

use super::*;

fn locator() -> Value {
    json!({ "selectors": [{ "kind": "css", "path": "aside > button" }] })
}

#[test]
fn the_commands_and_tools_keep_their_names() {
    assert_eq!(
        [STEP_ADD, STEP_TOGGLE, STEP_TRY, PLAY_FROM],
        [
            "recorder.step_add",
            "recorder.step_toggle",
            "recorder.step_try",
            "recorder.play_from"
        ]
    );
    assert!(TOOLS.iter().all(|tool| tool.starts_with("recorder_step_")));
    // An agent never deletes a recording: no tool here does.
    assert!(!TOOLS.iter().any(|tool| tool.contains("delete")));
}

#[test]
fn a_picked_element_goes_into_the_actions_target() {
    let mut click = json!({ "type": "click" });
    with_target(&mut click, locator()).expect("a click has an element");
    assert_eq!(click["target"], locator());
    let mut check = json!({ "type": "assert", "assertion": { "kind": "visible" } });
    with_target(&mut check, locator()).expect("a visible check has an element");
    assert_eq!(check["assertion"]["target"], locator());
    let action: Action = serde_json::from_value(check).expect("a valid assert");
    assert!(matches!(action, Action::Assert { .. }));
}

#[test]
fn a_kind_without_an_element_takes_no_point() {
    for mut action in [
        json!({ "type": "navigate", "url": "https://example.test/" }),
        json!({ "type": "wait", "ms": 500 }),
        json!({ "type": "waitForUrl", "url": "https://example.test/" }),
        json!({ "type": "assert", "assertion": { "kind": "url", "url": "https://example.test/" } }),
        json!("click"),
    ] {
        assert!(
            matches!(
                with_target(&mut action, locator()),
                Err(CommandError::InvalidArgs(_))
            ),
            "{action}"
        );
    }
}

#[test]
fn the_add_arguments_name_the_recording_and_the_action() {
    let args: AddArgs = serde_json::from_value(json!({
        "recordingId": "search", "after": 3, "pauseMs": 250,
        "action": { "type": "wait", "ms": 800 }, "onlyIn": ["mobile"], "name": "Settle"
    }))
    .expect("arguments");
    assert_eq!(args.recording_id, "search");
    assert_eq!((args.after, args.pause_ms), (Some(3), Some(250)));
    assert!(serde_json::from_value::<AddArgs>(json!({ "recordingId": "search" })).is_err());
    assert!(
        serde_json::from_value::<AddArgs>(
            json!({ "recordingId": "s", "action": {}, "delete": true })
        )
        .is_err()
    );
}

#[test]
fn play_from_here_plays_the_recording_from_the_step_on() {
    let args = play_from_args(&json!({ "recordingId": "search", "stepId": 4, "pace": "recorded" }))
        .expect("arguments");
    assert_eq!(
        args,
        json!({ "recordingId": "search", "fromStep": 4, "pace": "recorded" })
    );
    assert!(play_from_args(&json!({ "recordingId": "search" })).is_err());
}

/// RB-092-01 security review: an agent's secret fill is refused with its own MCP code, and a
/// command sees it as bad arguments.
#[test]
fn a_secret_fill_from_an_agent_is_refused_with_its_own_code() {
    let error = refused(edit::EditError::Secret(4));
    assert!(matches!(error, StepError::Secret(_)));
    let failure = ToolFailure::from(refused(edit::EditError::Secret(4)));
    assert_eq!(failure.code, SECRET_CODE);
    assert!(failure.message.contains("step 4"), "{}", failure.message);
    assert!(matches!(
        CommandError::from(refused(edit::EditError::Secret(4))),
        CommandError::InvalidArgs(_)
    ));
    let other = ToolFailure::from(refused(edit::EditError::NoStep(9)));
    assert_eq!(other.code, "invalid_arguments");
}

#[test]
fn an_agents_secret_fill_is_refused_before_anything_is_read() {
    // `add` checks the action before it opens the store: no project is needed to see it.
    let action: Action = serde_json::from_value(json!({
        "type": "fill", "target": { "selectors": [{ "kind": "css", "path": "#password" }] },
        "value": "{{secret:password}}", "secret": true
    }))
    .expect("a secret fill");
    assert_eq!(
        edit::check_secret(0, None, &action),
        Err(edit::EditError::Secret(0))
    );
}
