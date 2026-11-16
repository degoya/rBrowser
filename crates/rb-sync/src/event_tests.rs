use serde_json::json;

use super::*;

fn centre(path: &str) -> ElementTarget {
    ElementTarget {
        hosts: Vec::new(),
        element: ElementSelector::Css { path: path.into() },
        offset: Offset { x: 0.5, y: 0.5 },
    }
}

#[test]
fn a_selector_omits_nth_zero_and_reads_it_missing() {
    let selector = ElementSelector::Testid {
        testid: "save".into(),
        nth: 0,
        path: "#form > button".into(),
    };
    let json = serde_json::to_value(&selector).expect("serializes");
    assert_eq!(
        json,
        json!({ "kind": "testid", "testid": "save", "path": "#form > button" })
    );
    let back: ElementSelector = serde_json::from_value(json).expect("parses");
    assert_eq!(back, selector);
}

#[test]
fn a_target_omits_empty_hosts() {
    let json = serde_json::to_value(centre("#q")).expect("serializes");
    assert!(json.get("hosts").is_none());
}

#[test]
fn events_are_tagged_by_type_in_camel_case() {
    let event = SyncEvent::Wheel(WheelData {
        delta_x: 0.5,
        delta_y: 120.5,
        delta_mode: 0,
        modifiers: MOD_SHIFT,
        target: centre("main"),
    });
    let json = serde_json::to_value(&event).expect("serializes");
    assert_eq!(json["type"], "wheel");
    assert_eq!(json["deltaY"], 120.5);
    assert_eq!(json["modifiers"], 8);
}

#[test]
fn a_document_scroll_has_no_container() {
    let event: SyncEvent = serde_json::from_value(
        json!({ "type": "scroll", "x": 0.5, "y": 640.5, "maxX": 0.5, "maxY": 2400.5 }),
    )
    .expect("parses");
    assert!(matches!(
        event,
        SyncEvent::Scroll(ScrollData {
            container: None,
            ..
        })
    ));
}

#[test]
fn a_scroll_carries_its_anchor_and_omits_a_missing_one() {
    let event: SyncEvent = serde_json::from_value(json!({
        "type": "scroll", "x": 0.0, "y": 640.5, "maxX": 0.0, "maxY": 2400.5,
        "anchor": { "element": { "kind": "css", "path": "#s5 > h2" },
                    "offset": { "x": -0.25, "y": 0.125 } }
    }))
    .expect("parses");
    let SyncEvent::Scroll(scroll) = &event else {
        panic!("not a scroll: {event:?}");
    };
    let anchor = scroll.anchor.as_ref().expect("the anchor");
    assert_eq!(
        anchor.element,
        ElementSelector::Css {
            path: "#s5 > h2".into()
        }
    );
    assert_eq!(anchor.offset, Offset { x: -0.25, y: 0.125 });
    let plain = SyncEvent::Scroll(ScrollData {
        anchor: None,
        ..scroll.clone()
    });
    let json = serde_json::to_value(&plain).expect("serializes");
    assert!(json.get("anchor").is_none());
}

#[test]
fn debug_output_masks_every_typed_value() {
    let input = SyncEvent::Input(InputData {
        input_type: "insertText".into(),
        data: Some("x".into()),
        value: Some("hunter2x".into()),
        sensitive: true,
        target: centre("#password"),
    });
    let change = SyncEvent::Change(ChangeData {
        value: Some("hunter2".into()),
        checked: None,
        selected: None,
        sensitive: true,
        target: centre("#password"),
    });
    let text = format!("{input:?} {change:?}");
    assert!(!text.contains("hunter2"), "{text}");
    assert!(text.contains("<masked>"), "{text}");
}

#[test]
fn debug_output_masks_the_keys_of_a_password_field() {
    let key = |sensitive| KeyData {
        key: "q".into(),
        code: "KeyQ".into(),
        key_code: 81,
        location: 0,
        repeat: false,
        modifiers: 0,
        sensitive,
        target: Some(centre("#password")),
    };
    let secret = format!(
        "{:?} {:?}",
        SyncEvent::Keydown(key(true)),
        SyncEvent::Keyup(key(true))
    );
    assert!(!secret.contains("\"q\""), "{secret}");
    assert!(!secret.contains("KeyQ"), "{secret}");
    assert!(!secret.contains("81"), "{secret}");
    assert!(secret.contains("<masked>"), "{secret}");
    let plain = format!("{:?}", key(false));
    assert!(plain.contains("KeyQ"), "{plain}");
}

#[test]
fn a_sensitive_key_says_so_on_the_wire_and_a_plain_one_does_not() {
    let event: SyncEvent = serde_json::from_value(json!({
        "type": "keydown", "key": "q", "code": "KeyQ", "keyCode": 81, "location": 0,
        "repeat": false, "modifiers": 0, "sensitive": true
    }))
    .expect("parses");
    let SyncEvent::Keydown(data) = &event else {
        panic!("a keydown");
    };
    assert!(data.sensitive);
    let plain: SyncEvent = serde_json::from_value(json!({
        "type": "keyup", "key": "q", "code": "KeyQ", "keyCode": 81, "location": 0,
        "repeat": false, "modifiers": 0
    }))
    .expect("parses");
    let back = serde_json::to_value(&plain).expect("serializes");
    assert!(back.get("sensitive").is_none());
}

#[test]
fn an_unknown_event_type_is_refused() {
    let result = serde_json::from_value::<SyncEvent>(json!({ "type": "teleport" }));
    assert!(result.is_err());
}
