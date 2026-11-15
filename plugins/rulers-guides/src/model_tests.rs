use super::*;
use serde_json::json;

fn guide(id: u32, axis: Axis, unit: GuideUnit, value: f64) -> Guide {
    Guide {
        id,
        axis,
        unit,
        value,
    }
}

#[test]
fn a_guide_is_held_to_its_unit_and_range() {
    assert!(guide(1, Axis::X, GuideUnit::Percent, 50.0).check().is_ok());
    assert!(guide(1, Axis::X, GuideUnit::PxRight, 320.0).check().is_ok());
    assert!(guide(1, Axis::Y, GuideUnit::Px, 320.0).check().is_ok());
    assert_eq!(
        guide(1, Axis::X, GuideUnit::Percent, 100.5).check(),
        Err("guide.value")
    );
    assert_eq!(
        guide(1, Axis::X, GuideUnit::Px, f64::NAN).check(),
        Err("guide.value")
    );
    assert_eq!(
        guide(1, Axis::X, GuideUnit::Px, -1.0).check(),
        Err("guide.value")
    );
    assert_eq!(
        guide(1, Axis::Y, GuideUnit::Percent, 10.0).check(),
        Err("guide.unit")
    );
}

#[test]
fn guides_are_counted_and_their_ids_unique() {
    let two = [
        guide(1, Axis::X, GuideUnit::Px, 1.0),
        guide(1, Axis::Y, GuideUnit::Px, 2.0),
    ];
    assert_eq!(check_guides(&two), Err("guide.id"));
    let many: Vec<Guide> = (0..=MAX_GUIDES as u32)
        .map(|id| guide(id, Axis::Y, GuideUnit::Px, 1.0))
        .collect();
    assert_eq!(check_guides(&many), Err("guides"));
}

#[test]
fn a_column_setup_is_held_to_its_range() {
    let spec = |value: serde_json::Value| -> ColumnSpec {
        serde_json::from_value(value).expect("a column spec")
    };
    assert!(
        spec(json!({"columns": 12, "gutter": 24, "maxWidth": 1200}))
            .check()
            .is_ok()
    );
    assert_eq!(spec(json!({"columns": 0})).check(), Err("columns.columns"));
    assert_eq!(spec(json!({"columns": 49})).check(), Err("columns.columns"));
    assert_eq!(spec(json!({"gutter": -2})).check(), Err("columns.gutter"));
    assert_eq!(
        spec(json!({"maxWidth": 0})).check(),
        Err("columns.maxWidth")
    );
    assert!(serde_json::from_value::<ColumnSpec>(json!({"colums": 4})).is_err());
}

#[test]
fn breakpoint_keys_are_the_config_alphabet_or_any() {
    assert!(is_breakpoint_key("mobile"));
    assert!(is_breakpoint_key("tablet-landscape_2"));
    assert!(is_breakpoint_key(ANY_BREAKPOINT));
    assert!(!is_breakpoint_key(""));
    assert!(!is_breakpoint_key("a b"));
    assert!(!is_breakpoint_key("<script>"));
    assert!(!is_breakpoint_key(&"a".repeat(65)));
}

#[test]
fn the_baseline_step_is_bounded() {
    assert!(Baseline::default().check().is_ok());
    let tiny = Baseline {
        line_height: 1.0,
        offset: 0.0,
    };
    assert_eq!(tiny.check(), Err("baseline.lineHeight"));
}

#[test]
fn overrides_win_per_field_and_per_breakpoint() {
    let shared = Shared {
        visible: Visible {
            rulers: true,
            columns: false,
            baseline: false,
        },
        guides: vec![guide(1, Axis::Y, GuideUnit::Px, 100.0)],
        columns: BTreeMap::from([
            (
                "mobile".to_owned(),
                ColumnSpec {
                    columns: Some(4),
                    ..ColumnSpec::default()
                },
            ),
            (
                "desktop".to_owned(),
                ColumnSpec {
                    columns: Some(12),
                    ..ColumnSpec::default()
                },
            ),
        ]),
        baseline: Baseline::default(),
    };
    let overrides = Overrides {
        visible: VisibleOverrides {
            columns: Some(true),
            ..VisibleOverrides::default()
        },
        guides: None,
        columns: BTreeMap::from([
            (
                "mobile".to_owned(),
                Some(ColumnSpec {
                    columns: Some(6),
                    ..ColumnSpec::default()
                }),
            ),
            ("desktop".to_owned(), None),
        ]),
        baseline: None,
    };
    let state = effective(&shared, &overrides);
    assert_eq!(
        state.visible,
        Visible {
            rulers: true,
            columns: true,
            baseline: false
        }
    );
    assert_eq!(state.guides, shared.guides);
    assert_eq!(state.columns["mobile"].columns, Some(6));
    assert!(!state.columns.contains_key("desktop"));
}

#[test]
fn overrides_refuse_a_foreign_breakpoint_key_and_unknown_fields() {
    let overrides: Overrides =
        serde_json::from_value(json!({"columns": {"a/b": {"columns": 2}}})).expect("overrides");
    assert_eq!(overrides.check(), Err("columns.breakpoint"));
    assert!(serde_json::from_value::<Overrides>(json!({"script": "x"})).is_err());
}

#[test]
fn the_state_reads_back_in_its_wire_form() {
    let text = r#"{"visible":{"rulers":true,"columns":false,"baseline":true},
        "guides":[{"id":3,"axis":"x","unit":"percent","value":50}],
        "columns":{"*":{"preset":"bootstrap","maxWidth":1200}},
        "baseline":{"lineHeight":8,"offset":4}}"#;
    let state: State = serde_json::from_str(text).expect("the state");
    assert_eq!(state.guides[0].unit, GuideUnit::Percent);
    assert_eq!(state.columns["*"].preset, Some(Preset::Bootstrap));
    assert_eq!(state.columns["*"].max_width, Some(1200.0));
    let again: State =
        serde_json::from_str(&serde_json::to_string(&state).expect("json")).expect("again");
    assert_eq!(again, state);
}
