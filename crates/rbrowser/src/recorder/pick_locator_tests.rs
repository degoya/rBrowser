use serde_json::json;

use super::*;

fn all() -> Vec<(ViewId, String)> {
    vec![
        (ViewId(1), "mobile".to_owned()),
        (ViewId(2), "tablet".to_owned()),
        (ViewId(3), "desktop".to_owned()),
    ]
}

#[test]
fn an_element_every_view_has_runs_in_all_of_them() {
    assert!(only_in(&[ViewId(2), ViewId(1), ViewId(3)], &all()).is_empty());
}

#[test]
fn an_element_some_views_lack_runs_in_the_others_in_layout_order() {
    assert_eq!(
        only_in(&[ViewId(3), ViewId(2)], &all()),
        ["tablet", "desktop"]
    );
}

#[test]
fn a_point_needs_both_coordinates_and_no_point_is_a_pick() {
    assert_eq!(point_of(None, None, None).expect("a pick"), None);
    let (point, dip) = point_of(Some(4.0), Some(8.5), Some(true))
        .expect("a point")
        .expect("some");
    assert_eq!((point.x, point.y, dip), (4.0, 8.5, true));
    assert!(point_of(Some(4.0), None, None).is_err());
    assert!(point_of(Some(-1.0), Some(2.0), None).is_err());
    assert!(point_of(Some(f64::NAN), Some(2.0), None).is_err());
}

#[test]
fn a_selects_options_are_named_and_capped() {
    let many: Vec<_> = (0..250)
        .map(|n| json!({ "value": format!("v{n}"), "label": format!("Size {n}") }))
        .collect();
    let options = options_of(&json!({ "target": {}, "options": many }));
    assert_eq!(options.len(), MAX_OPTIONS);
    assert_eq!(
        options[1],
        SelectOption {
            value: "v1".to_owned(),
            label: "Size 1".to_owned()
        }
    );
    assert!(options_of(&json!({ "target": {}, "options": [] })).is_empty());
    assert!(options_of(&json!({ "target": {} })).is_empty());
    assert!(options_of(&json!({ "options": "nonsense" })).is_empty());
}

#[test]
fn the_agents_answer_gives_the_target_and_the_text() {
    let answer = json!({ "result": { "value": {
        "target": { "element": { "kind": "css", "path": "main" } }, "text": "Welcome", "options": []
    } } });
    let (target, text) = described(&answer).expect("described");
    assert_eq!(text, "Welcome");
    assert_eq!(target["element"]["path"], "main");
    assert!(described(&json!({ "exceptionDetails": {} })).is_err());
}

#[test]
fn the_describe_script_never_reads_a_fields_value() {
    // A picked field names its options and text; what was typed into it stays in the page.
    assert!(!DESCRIBE.contains("this.value"));
    assert!(DESCRIBE.contains("targetOf(this)"));
}

#[test]
fn the_command_keeps_its_name() {
    assert_eq!(PICK_ELEMENT, "recorder.pick_element");
}

#[test]
fn the_panels_answer_holds_the_locator_text_views_and_options() {
    let target: ElementTarget = serde_json::from_value(json!({
        "element": { "kind": "role", "role": "button", "name": "Filter", "nth": 0,
                     "path": "aside > button" },
        "offset": { "x": 0.5, "y": 0.5 }
    }))
    .expect("an agent target");
    let picked = Picked {
        source: ViewId(2),
        target,
        text: "x".repeat(600),
        views: vec![ViewId(2), ViewId(3)],
        options: vec![SelectOption {
            value: "42".to_owned(),
            label: "Size 42".to_owned(),
        }],
    };
    let answer = answer_of(&picked, &all());
    assert_eq!(answer.view, 2);
    assert_eq!(answer.locator["selectors"][0]["role"], "button");
    assert_eq!(answer.text.chars().count(), super::super::check::MAX_TEXT);
    assert_eq!(answer.only_in, ["tablet", "desktop"]);
    assert_eq!(
        answer.options,
        [json!({ "value": "42", "label": "Size 42" })]
    );
}

#[test]
fn the_panels_pick_names_a_recording_a_file_can_have() {
    let args = pick_args(&json!({ "recordingId": "search", "view": 2 })).expect("arguments");
    assert_eq!(args.recording_id.as_deref(), Some("search"));
    assert_eq!(args.view, Some(2));
    assert!(pick_args(&json!({ "recordingId": "../x" })).is_err());
    assert!(pick_args(&json!({ "recordingId": "search", "colour": 1 })).is_err());
}
