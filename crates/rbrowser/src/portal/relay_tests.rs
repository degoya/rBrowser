use rb_sync::{
    ElementSelector, ElementTarget, InputData, MouseButton, MouseData, Offset, TargetData,
};
use serde_json::json;

use super::*;

fn target() -> ElementTarget {
    ElementTarget {
        hosts: Vec::new(),
        element: ElementSelector::Css {
            path: "#go".to_owned(),
        },
        offset: Offset { x: 0.5, y: 0.5 },
    }
}

fn click() -> SyncEvent {
    SyncEvent::Click(MouseData {
        button: MouseButton::Left,
        detail: 1,
        modifiers: 0,
        target: target(),
    })
}

fn on() -> SyncSwitches {
    SyncSwitches::default()
}

#[test]
fn a_click_goes_to_the_device_as_it_is() {
    let message = for_device(&click(), &on()).expect("replayable");
    let json = serde_json::to_value(&message).expect("serialises");
    assert_eq!(json["kind"], "event");
    assert_eq!(json["event"]["type"], "click");
}

#[test]
fn hover_keys_and_pointers_cannot_be_replayed_by_script() {
    let hover = SyncEvent::Hover(TargetData { target: target() });
    assert_eq!(for_device(&hover, &on()), Err(Skip::NotReplayable));
}

#[test]
fn a_password_never_leaves_the_machine() {
    let password = SyncEvent::Input(InputData {
        input_type: "insertText".to_owned(),
        data: Some("x".to_owned()),
        value: Some("secret".to_owned()),
        sensitive: true,
        target: target(),
    });
    assert_eq!(for_device(&password, &on()), Err(Skip::Sensitive));
}

#[test]
fn a_kind_switched_off_is_not_sent() {
    let mut switches = on();
    switches.click = false;
    assert_eq!(for_device(&click(), &switches), Err(Skip::Off));
    let mut switches = on();
    switches.enabled = false;
    assert_eq!(for_device(&click(), &switches), Err(Skip::Off));
}

#[test]
fn navigation_carries_the_path_only() {
    let message = navigate("http://localhost:5173/a?b#c").expect("followable");
    assert_eq!(
        serde_json::to_value(&message).expect("serialises"),
        json!({ "kind": "navigate", "path": "/a?b#c" })
    );
    assert_eq!(navigate("about:blank"), None);
}

#[test]
fn the_device_reports_parse_and_junk_is_refused() {
    assert_eq!(
        parse_report(r#"{"kind":"replayed","status":"done"}"#),
        Some(Report::Replayed {
            status: ReplayStatus::Done
        })
    );
    assert_eq!(
        parse_report(r#"{"kind":"replayed","status":"unsupported"}"#),
        Some(Report::Replayed {
            status: ReplayStatus::Unsupported
        })
    );
    assert_eq!(parse_report(r#"{"kind":"hello"}"#), Some(Report::Hello));
    assert_eq!(parse_report(r#"{"kind":"navigate","url":"x"}"#), None);
    assert_eq!(parse_report(&"x".repeat(MAX_REPORT + 1)), None);
}

#[test]
fn counters_add_up() {
    let mut counts = Counts::default();
    counts.note(&Report::Replayed {
        status: ReplayStatus::Done,
    });
    counts.note(&Report::Replayed {
        status: ReplayStatus::NotFound,
    });
    counts.skipped(Skip::NotReplayable);
    assert_eq!((counts.replayed, counts.failed, counts.skipped), (1, 1, 1));
}
