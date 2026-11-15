use rb_engine::{Target, ViewId};
use rb_sync::{MOD_CTRL, MOD_SHIFT, ReplayOutcome, SyncEvent};
use serde_json::json;

use super::*;

#[test]
fn the_names_are_the_tool_names_rb_030_01_registers() {
    assert_eq!(
        [CLICK_TOOL, TYPE_TOOL, HOVER_TOOL, PRESS_KEY_TOOL],
        ["click", "type", "hover", "press_key"]
    );
    for description in [
        CLICK_DESCRIPTION,
        TYPE_DESCRIPTION,
        HOVER_DESCRIPTION,
        PRESS_KEY_DESCRIPTION,
    ] {
        assert!(description.contains("target"));
    }
}

#[test]
fn click_takes_a_selector_and_defaults_to_one_left_click() {
    let args: ClickArgs = parse(&json!({ "selector": "#save" })).expect("arguments");
    assert_eq!(args.target, None);
    assert_eq!(args.button, ClickButton::Left);
    assert_eq!(args.click_count, 1);
    let args: ClickArgs = parse(&json!({
        "target": 2, "selector": "#save", "button": "right", "clickCount": 2,
        "modifiers": ["Control", "Shift"],
    }))
    .expect("arguments");
    assert_eq!(args.target, Some(Target::View(ViewId(2))));
    let events = click_events(&args).expect("events");
    assert_eq!(events.len(), 2);
    match &events[1] {
        SyncEvent::Click(click) => {
            assert_eq!(click.detail, 2);
            assert_eq!(click.modifiers, MOD_CTRL | MOD_SHIFT);
            assert_eq!(click.button, rb_sync::MouseButton::Right);
        }
        other => panic!("a click, not {other:?}"),
    }
}

#[test]
fn bad_arguments_are_refused_without_their_value() {
    let unknown = parse::<ClickArgs>(&json!({ "selector": "a", "x": 1 })).expect_err("refused");
    assert!(unknown.to_string().contains("unknown field"));
    let wrong = parse::<TypeArgs>(&json!({ "text": 4711 })).expect_err("refused");
    assert!(!wrong.to_string().contains("4711"));
    let count = ClickArgs {
        target: None,
        selector: "a".into(),
        button: ClickButton::Left,
        click_count: 4,
        modifiers: Vec::new(),
    };
    assert!(click_events(&count).is_err());
    let empty = HoverArgs {
        target: Some(Target::All),
        selector: "  ".into(),
    };
    assert!(hover_events(&empty).is_err());
}

#[test]
fn type_clicks_the_field_then_inserts_the_text() {
    let args: TypeArgs = parse(&json!({ "selector": "#pw", "text": "s3cret" })).expect("arguments");
    assert!(!format!("{args:?}").contains("s3cret"));
    let events = type_events(&args).expect("events");
    assert!(matches!(events[0], SyncEvent::Click(_)));
    match &events[1] {
        SyncEvent::Input(input) => {
            assert_eq!(input.input_type, "insertText");
            assert_eq!(input.data.as_deref(), Some("s3cret"));
        }
        other => panic!("an input, not {other:?}"),
    }
    // Without a selector the text goes into the focused element.
    let focused = TypeArgs {
        target: None,
        selector: None,
        text: "x".into(),
    };
    let events = type_events(&focused).expect("events");
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], SyncEvent::Input(_)));
    assert_eq!(rb_sync::selector_of(&events[0]).as_deref(), Some(":focus"));
}

#[test]
fn press_key_is_down_and_up_of_a_named_key() {
    let args: PressKeyArgs =
        parse(&json!({ "key": "Enter", "modifiers": ["Shift"] })).expect("arguments");
    let events = press_key_events(&args).expect("events");
    match (&events[0], &events[1]) {
        (SyncEvent::Keydown(down), SyncEvent::Keyup(up)) => {
            assert_eq!((down.key.as_str(), down.code.as_str()), ("Enter", "Enter"));
            assert_eq!(down.key_code, 13);
            assert_eq!(down.modifiers, MOD_SHIFT);
            assert_eq!(up.key, "Enter");
        }
        other => panic!("down and up, not {other:?}"),
    }
    let unknown = PressKeyArgs {
        target: None,
        key: "Hyper".into(),
        modifiers: Vec::new(),
    };
    assert!(press_key_events(&unknown).is_err());
}

#[test]
fn the_answer_has_a_status_per_view_and_never_the_input() {
    let answer = InputAnswer {
        views: vec![
            ViewInput::new(ViewId(1), ReplayOutcome::Done),
            ViewInput::new(ViewId(2), ReplayOutcome::NotFound),
            ViewInput::new(
                ViewId(3),
                ReplayOutcome::Failed("CDP Input.insertText failed".into()),
            ),
        ],
    };
    assert_eq!(
        serde_json::to_value(&answer).expect("json"),
        json!({ "views": [
            { "view": 1, "status": "done" },
            { "view": 2, "status": "notFound" },
            { "view": 3, "status": "failed", "message": "CDP Input.insertText failed" },
        ] })
    );
}

/// Review batch 3, finding 7: a view out of time says that its input may have gone out.
#[test]
fn a_timeout_says_the_input_may_have_been_sent() {
    let answer = ViewInput::new(ViewId(2), ReplayOutcome::Timeout);
    assert_eq!(answer.status, InputStatus::Timeout);
    let message = answer.message.expect("a message");
    assert!(message.contains("may have been sent"), "{message}");
    assert!(message.contains("mcp.navigationTimeoutMs"), "{message}");
    assert_eq!(ViewInput::new(ViewId(2), ReplayOutcome::Done).message, None);
}
