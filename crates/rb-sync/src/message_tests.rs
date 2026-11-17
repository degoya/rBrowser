use serde_json::json;

use super::*;

#[test]
fn parses_a_ready_message() {
    let message = parse_message(r#"{"kind":"ready","v":1,"top":true,"url":"http://x/"}"#)
        .expect("a valid message");
    assert_eq!(
        message,
        AgentMessage::Ready(Ready {
            v: 1,
            top: true,
            frame: None,
            url: "http://x/".into(),
        })
    );
}

#[test]
fn refuses_another_version() {
    let result = parse_message(r#"{"kind":"ready","v":2,"top":true,"url":"http://x/"}"#);
    assert_eq!(result, Err(ProtocolError::Version(2)));
}

#[test]
fn refuses_an_oversized_payload() {
    let payload = format!(
        r#"{{"kind":"ready","v":1,"top":true,"url":"{}"}}"#,
        "a".repeat(MAX_MESSAGE_BYTES)
    );
    assert!(matches!(
        parse_message(&payload),
        Err(ProtocolError::TooLarge(_))
    ));
}

#[test]
fn an_invalid_payload_is_not_quoted_in_the_error() {
    let payload = r#"{"kind":"event","v":1,"top":true,"seq":1,"time":1,"event":{"type":"input","inputType":"insertText","value":7,"sensitive":true,"target":"hunter2"}}"#;
    let error = parse_message(payload).expect_err("invalid");
    let text = error.to_string();
    assert!(!text.contains("hunter2"), "{text}");
    assert!(matches!(
        error,
        ProtocolError::Invalid {
            category: "data",
            ..
        }
    ));
}

#[test]
fn commands_are_tagged_by_cmd() {
    let replay = serde_json::to_value(AgentCommand::Replay { ms: 500 }).expect("serializes");
    assert_eq!(replay, json!({ "cmd": "replay", "ms": 500 }));
    let ping = serde_json::to_value(AgentCommand::Ping).expect("serializes");
    assert_eq!(ping, json!({ "cmd": "ping" }));
}

#[test]
fn results_use_snake_case_status() {
    let result: CommandResult =
        serde_json::from_value(json!({ "status": "not_visible" })).expect("parses");
    assert_eq!(result, CommandResult::NotVisible);
    let ok: CommandResult =
        serde_json::from_value(json!({ "status": "ok", "x": 1.5, "y": 2.5, "scrolled": false }))
            .expect("parses");
    assert_eq!(
        ok,
        CommandResult::Ok {
            x: 1.5,
            y: 2.5,
            scrolled: false
        }
    );
}

#[test]
fn the_config_is_camel_case() {
    let config = AgentConfig {
        active: false,
        capture: CaptureSwitches {
            click: true,
            hover: true,
            scroll: true,
            keyboard: true,
            input: true,
            forms: true,
            focus: false,
        },
        hover_throttle_ms: 100,
        scroll_debounce_ms: 150,
        scroll_mode: AgentScrollMode::Wheel,
    };
    let json = serde_json::to_value(config).expect("serializes");
    assert_eq!(json["hoverThrottleMs"], 100);
    assert_eq!(json["scrollDebounceMs"], 150);
    assert_eq!(json["scrollMode"], "wheel");
    assert_eq!(json["capture"]["keyboard"], true);
    assert_eq!(json["capture"]["forms"], true);
    assert_eq!(json["capture"]["focus"], false);
}

#[test]
fn apply_carries_the_change_and_masks_it_in_debug() {
    let command = AgentCommand::Apply(crate::event::ChangeData {
        value: Some("hunter2".into()),
        checked: None,
        selected: None,
        sensitive: true,
        target: crate::switches::tests::target("#password"),
    });
    let json = serde_json::to_value(&command).expect("serializes");
    assert_eq!(json["cmd"], "apply");
    assert_eq!(json["value"], "hunter2");
    assert_eq!(json["sensitive"], true);
    let text = format!("{command:?}");
    assert!(!text.contains("hunter2"), "{text}");
}

#[test]
fn focus_is_a_command_with_a_target() {
    let command = AgentCommand::Focus {
        target: crate::switches::tests::target("#q"),
    };
    let json = serde_json::to_value(&command).expect("serializes");
    assert_eq!(json["cmd"], "focus");
    assert_eq!(json["target"]["element"]["path"], "#q");
}

#[test]
fn parses_a_detected_message() {
    // RB-020-06: the agent found Browsersync's client in the page.
    let message = parse_message(r#"{"kind":"detected","v":1,"top":true,"tool":"browsersync"}"#)
        .expect("a valid message");
    assert_eq!(
        message,
        AgentMessage::Detected(Detected {
            v: 1,
            top: true,
            frame: None,
            tool: DetectedTool::Browsersync,
        })
    );
    assert_eq!(message.version(), 1);
    assert!(parse_message(r#"{"kind":"detected","v":1,"top":true,"tool":"livereload"}"#).is_err());
}

#[test]
fn the_navigate_command_carries_the_url() {
    // RB-020-06: a SPA route follows through the history API in the follower's document.
    let navigate = AgentCommand::Navigate {
        url: "http://localhost:5173/about".into(),
    };
    assert_eq!(
        serde_json::to_value(navigate).expect("serializes"),
        json!({ "cmd": "navigate", "url": "http://localhost:5173/about" })
    );
}

#[test]
fn a_config_without_the_scroll_debounce_takes_its_default() {
    let config: AgentConfig = serde_json::from_value(serde_json::json!({
        "active": true,
        "capture": { "click": true, "hover": true, "scroll": true, "keyboard": true, "input": true, "forms": true, "focus": false },
        "hoverThrottleMs": 100
    }))
    .expect("parses");
    assert_eq!(config.scroll_debounce_ms, DEFAULT_SCROLL_DEBOUNCE_MS);
    // RB-090-33: without a mode the agent listens for no wheel (anchor mode).
    assert_eq!(config.scroll_mode, AgentScrollMode::Anchor);
}

#[test]
fn the_scroll_command_carries_the_scroll_flat() {
    let command: AgentCommand = serde_json::from_value(serde_json::json!({
        "cmd": "scroll", "x": 0.0, "y": 640.5, "maxX": 0.0, "maxY": 2400.0,
        "anchor": { "element": { "kind": "css", "path": "#s5" }, "offset": { "x": 0.0, "y": 0.25 } }
    }))
    .expect("parses");
    let AgentCommand::Scroll(scroll) = &command else {
        panic!("not a scroll: {command:?}");
    };
    assert_eq!(scroll.y, 640.5);
    assert!(scroll.container.is_none());
    assert_eq!(
        scroll.anchor.as_ref().map(|anchor| anchor.offset.y),
        Some(0.25)
    );
    let json = serde_json::to_value(&command).expect("serializes");
    assert_eq!(json["cmd"], "scroll");
    assert_eq!(json["maxY"], 2400.0);
}

#[test]
fn the_point_command_is_tagged_point() {
    let command = AgentCommand::Point {
        target: crate::event::ElementTarget {
            hosts: Vec::new(),
            element: crate::event::ElementSelector::Css {
                path: "main".into(),
            },
            offset: crate::event::Offset { x: 0.5, y: 0.5 },
        },
    };
    let json = serde_json::to_value(&command).expect("serializes");
    assert_eq!(json["cmd"], "point");
    assert_eq!(json["target"]["element"]["path"], "main");
}
