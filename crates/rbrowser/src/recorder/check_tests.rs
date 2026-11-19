use rb_sync::recorder::Selector;
use serde_json::json;

use super::*;

fn target() -> ElementTarget {
    serde_json::from_value(json!({
        "element": { "kind": "testid", "testid": "welcome", "nth": 0, "path": "main > h1" },
        "offset": { "x": 0.5, "y": 0.5 }
    }))
    .expect("an agent target")
}

#[test]
fn the_page_menu_arguments_are_read() {
    let args =
        parse(&json!({ "kind": "text", "view": 2, "x": 30, "y": 40, "dip": true })).expect("valid");
    assert_eq!(args.kind, CheckKind::Text);
    assert_eq!(args.view, Some(ViewId(2)));
    assert_eq!(args.point, Some((Point { x: 30.0, y: 40.0 }, true)));
    // The shortcut: a visible check on the element the next click picks.
    let pick = parse(&Value::Null).expect("no arguments");
    assert_eq!(pick.kind, CheckKind::Visible);
    assert_eq!(pick.point, None);
    assert_eq!(
        parse(&json!({ "kind": "waitFor" })).expect("valid").kind,
        CheckKind::WaitFor
    );
}

#[test]
fn bad_arguments_are_refused() {
    for bad in [
        json!({ "x": 3 }),
        json!({ "x": -1, "y": 2 }),
        json!({ "kind": "colour" }),
        json!({ "selector": "h1" }),
    ] {
        assert!(
            matches!(parse(&bad), Err(CommandError::InvalidArgs(_))),
            "{bad}"
        );
    }
}

#[test]
fn each_kind_is_its_step_with_the_click_syncs_selectors() {
    let visible = action_of(CheckKind::Visible, &target(), "").expect("a step");
    let Action::Assert {
        assertion: Assertion::Visible { target: locator },
    } = &visible
    else {
        panic!("{visible:?}");
    };
    assert_eq!(
        locator.selectors,
        vec![
            Selector::Testid {
                testid: "welcome".into(),
                nth: 0
            },
            Selector::Css {
                path: "main > h1".into()
            },
        ]
    );
    assert!(matches!(
        action_of(CheckKind::Text, &target(), "Welcome back"),
        Ok(Action::Assert {
            assertion: Assertion::Text { text, .. }
        }) if text == "Welcome back"
    ));
    assert!(matches!(
        action_of(CheckKind::WaitFor, &target(), ""),
        Ok(Action::WaitFor { .. })
    ));
}

#[test]
fn a_text_check_needs_text_and_keeps_at_most_its_limit() {
    assert!(action_of(CheckKind::Text, &target(), "  ").is_err());
    let long = "x".repeat(MAX_TEXT + 20);
    let Ok(Action::Assert {
        assertion: Assertion::Text { text, .. },
    }) = action_of(CheckKind::Text, &target(), &long)
    else {
        panic!("a text step");
    };
    assert_eq!(text.chars().count(), MAX_TEXT);
}

#[test]
fn the_agents_answer_gives_the_target_and_the_text() {
    let answer = json!({ "result": { "value": {
        "target": { "element": { "kind": "css", "path": "h1" }, "offset": { "x": 0.5, "y": 0.5 } },
        "text": "Hello"
    } } });
    let (target, text) = described(&answer).expect("described");
    assert_eq!(target["element"]["path"], "h1");
    assert_eq!(text, "Hello");
    assert!(described(&json!({ "result": { "value": { "target": null, "text": "" } } })).is_err());
    assert!(described(&json!({ "exceptionDetails": {} })).is_err());
}

#[test]
fn a_check_that_waits_for_its_click_says_so() {
    // RB-090-84 (RB-090-85's audit): the capture's pick hint, no settings link.
    let CoreToShell::Notification(notice) = pick_notice() else {
        panic!("a notice");
    };
    assert_eq!(notice.message_key, "capture.notice.pick");
    assert!(notice.args.is_empty());
    assert_eq!(notice.settings_key, None);
}
