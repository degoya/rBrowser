use super::*;
use crate::recorder::model::{Action, Locator, RecordedBreakpoint, Selector, Step};

fn sample() -> Recording {
    Recording {
        schema: Some("../../schema/recording.v1.json".into()),
        version: RECORDING_VERSION,
        name: "login".into(),
        start_url: Some("http://x.test/".into()),
        environment: Some("local".into()),
        breakpoints: vec![RecordedBreakpoint {
            id: "mobile".into(),
            width: 390,
            height: 844,
            device_scale_factor: 3.0,
        }],
        started_at_ms: 42,
        app_version: "0.1.0".into(),
        steps: vec![Step {
            id: 1,
            name: Some("Log in".into()),
            action: Action::Fill {
                target: Locator {
                    selectors: vec![Selector::Css {
                        path: "#password".into(),
                    }],
                    hosts: Vec::new(),
                    frame: None,
                },
                value: "{{secret:password}}".into(),
                secret: true,
            },
            view: "mobile".into(),
            at_ms: 10,
            only_in: vec!["mobile".into()],
            path: Vec::new(),
            disabled: false,
            pause_ms: None,
        }],
    }
}

#[test]
fn the_json_exporter_round_trips() {
    let exporter: &dyn Exporter = &JsonExporter;
    assert_eq!((exporter.id(), exporter.extension()), ("json", "json"));
    let text = exporter.export(&sample()).expect("exports");
    assert!(text.ends_with('\n'));
    assert_eq!(from_json(&text).expect("reads"), sample());
}

#[test]
fn another_version_or_no_recording_is_refused() {
    let mut value = serde_json::to_value(sample()).expect("serialises");
    value["version"] = 2.into();
    assert!(matches!(
        from_json(&value.to_string()),
        Err(ExportError::Version(2))
    ));
    assert!(matches!(from_json("{}"), Err(ExportError::Invalid(_))));
    assert!(matches!(from_json("nope"), Err(ExportError::Invalid(_))));
}
