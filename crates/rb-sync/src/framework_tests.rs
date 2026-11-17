use super::*;
use crate::{AgentMessage, parse_message};

#[test]
fn parses_a_framework_message_and_writes_it_back_unchanged() {
    let text = r#"{"kind":"framework","v":1,"top":true,"vue":{"version":"3.5.22","hook":true,"devtools":true},"react":{"hook":false,"devtools":false}}"#;
    let message = parse_message(text).expect("a valid message");
    let AgentMessage::Framework(report) = &message else {
        panic!("not a framework message: {message:?}");
    };
    assert_eq!(message.version(), 1);
    assert_eq!(
        report.vue,
        Some(FrameworkInfo {
            version: Some("3.5.22".into()),
            hook: true,
            devtools: true,
        })
    );
    assert_eq!(
        report.info(Framework::React).map(|info| info.hook),
        Some(false)
    );
    assert_eq!(
        report.detected().collect::<Vec<_>>(),
        [Framework::Vue, Framework::React]
    );
    let back = serde_json::to_string(&message).expect("serializes");
    assert_eq!(back, text);
}

#[test]
fn an_empty_report_names_no_framework() {
    let message =
        parse_message(r#"{"kind":"framework","v":1,"top":true}"#).expect("a valid message");
    let AgentMessage::Framework(report) = message else {
        panic!("not a framework message");
    };
    assert_eq!(report.detected().count(), 0);
    assert_eq!(report.info(Framework::Vue), None);
}

#[test]
fn a_report_needs_both_booleans_and_knows_no_other_framework() {
    assert!(parse_message(r#"{"kind":"framework","v":1,"top":true,"vue":{"hook":true}}"#).is_err());
    assert!(
        parse_message(
            r#"{"kind":"framework","v":1,"top":true,"react":{"hook":"yes","devtools":true}}"#
        )
        .is_err()
    );
    // An unknown framework is ignored, as serde skips unknown fields: the core never learns it.
    let message = parse_message(
        r#"{"kind":"framework","v":1,"top":true,"svelte":{"hook":true,"devtools":true}}"#,
    )
    .expect("reads");
    let AgentMessage::Framework(report) = message else {
        panic!("not a framework message");
    };
    assert_eq!(report.detected().count(), 0);
}

#[test]
fn framework_ids_are_the_settings_and_command_names() {
    assert_eq!(Framework::Vue.id(), "vue");
    assert_eq!(Framework::React.id(), "react");
    assert_eq!(
        serde_json::to_value(Framework::React).expect("serializes"),
        serde_json::Value::from("react")
    );
}
