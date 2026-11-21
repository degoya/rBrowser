use rb_bidi::input::actions_for;
use rb_engine::{Button, InputEvent, Phase};
use serde_json::json;

use super::*;

fn mouse(phase: Phase, button: Button) -> InputEvent {
    InputEvent::Mouse {
        phase,
        x: 10.0,
        y: 20.0,
        button,
        modifiers: 0,
        clicks: 1,
    }
}

#[test]
fn a_click_becomes_a_touch_of_one_finger() {
    for phase in [Phase::Down, Phase::Up] {
        let sources = touch_sources(actions_for(&mouse(phase, Button::Left)).expect("sources"));
        assert_eq!(sources.len(), 1, "{sources:?}");
        assert_eq!(sources[0]["type"], "pointer");
        assert_eq!(sources[0]["id"], "finger");
        assert_eq!(sources[0]["parameters"]["pointerType"], "touch");
        assert_eq!(sources[0]["actions"][0]["type"], "pointerMove");
    }
}

#[test]
fn hover_and_the_wheel_have_no_touch() {
    let hover = actions_for(&mouse(Phase::Move, Button::None)).expect("sources");
    assert!(touch_sources(hover).is_empty());
    let wheel = InputEvent::Wheel {
        x: 1.0,
        y: 2.0,
        delta_x: 0.0,
        delta_y: 120.0,
    };
    assert!(touch_sources(actions_for(&wheel).expect("sources")).is_empty());
}

#[test]
fn keys_and_text_stay_keys() {
    let text = InputEvent::Text {
        text: "ab".to_owned(),
    };
    let sources = actions_for(&text).expect("sources");
    assert_eq!(touch_sources(sources.clone()), sources);
    let other = vec![json!({ "type": "none", "id": "pause", "actions": [] })];
    assert_eq!(touch_sources(other.clone()), other);
}
