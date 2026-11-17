use super::*;
use crate::model::{Align, ColumnSpec, Preset};

fn spec(columns: u32, gutter: f64, margin: f64, max_width: Option<f64>) -> ColumnSpec {
    ColumnSpec {
        columns: Some(columns),
        gutter: Some(gutter),
        margin: Some(margin),
        max_width,
        ..ColumnSpec::default()
    }
}

fn xs(layout: &ColumnLayout) -> Vec<(f64, f64)> {
    layout
        .columns
        .iter()
        .map(|column| (column.x, column.width))
        .collect()
}

#[test]
fn mobile_four_columns_with_a_16_px_gutter_fill_the_page() {
    // The acceptance fixture's mobile setup: 4 columns, 16 px gutter, 16 px margin, at 375 px.
    let layout = layout(&resolve(&spec(4, 16.0, 16.0, None), 375.0), 375.0);
    assert_eq!(layout.left, 0.0);
    assert_eq!(layout.width, 375.0);
    // (375 - 2 * 16 - 3 * 16) / 4 = 73.75
    assert_eq!(
        xs(&layout),
        vec![
            (16.0, 73.75),
            (105.75, 73.75),
            (195.5, 73.75),
            (285.25, 73.75)
        ]
    );
}

#[test]
fn desktop_twelve_columns_are_centred_in_their_max_width() {
    // 12 columns, 24 px gutter, 12 px margin, at most 1200 px, in a 1440 px page: Bootstrap's
    // container (padding 12 px) and row (gutter 24 px).
    let layout = layout(
        &resolve(&spec(12, 24.0, 12.0, Some(1200.0)), 1440.0),
        1440.0,
    );
    assert_eq!(layout.left, 120.0);
    assert_eq!(layout.width, 1200.0);
    let columns = xs(&layout);
    assert_eq!(columns.len(), 12);
    // (1200 - 24 - 11 * 24) / 12 = 76; a Bootstrap .col-1 is 100 px wide with 12 px padding.
    assert_eq!(columns[0], (132.0, 76.0));
    assert_eq!(columns[1], (232.0, 76.0));
    assert_eq!(columns[11], (1232.0, 76.0));
}

#[test]
fn a_narrower_page_than_the_max_width_takes_the_page() {
    let layout = layout(&resolve(&spec(12, 24.0, 12.0, Some(1200.0)), 800.0), 800.0);
    assert_eq!((layout.left, layout.width), (0.0, 800.0));
}

#[test]
fn align_left_and_right_place_the_box() {
    let mut left = spec(2, 0.0, 0.0, Some(100.0));
    left.align = Some(Align::Left);
    assert_eq!(layout(&resolve(&left, 300.0), 300.0).left, 0.0);
    let mut right = left.clone();
    right.align = Some(Align::Right);
    assert_eq!(layout(&resolve(&right, 300.0), 300.0).left, 200.0);
}

#[test]
fn gutters_wider_than_the_box_leave_columns_of_no_width() {
    let layout = layout(&resolve(&spec(12, 100.0, 0.0, None), 320.0), 320.0);
    assert!(layout.columns.iter().all(|column| column.width == 0.0));
}

#[test]
fn the_bootstrap_preset_follows_the_container_widths() {
    let bootstrap = ColumnSpec {
        preset: Some(Preset::Bootstrap),
        ..ColumnSpec::default()
    };
    let at = |width: f64| resolve(&bootstrap, width);
    assert_eq!(at(375.0).max_width, None);
    assert_eq!(at(576.0).max_width, Some(540.0));
    assert_eq!(at(800.0).max_width, Some(720.0));
    assert_eq!(at(1000.0).max_width, Some(960.0));
    assert_eq!(at(1280.0).max_width, Some(1140.0));
    assert_eq!(at(1440.0).max_width, Some(1320.0));
    let resolved = at(1280.0);
    assert_eq!(
        (resolved.columns, resolved.gutter, resolved.margin),
        (12, 24.0, 12.0)
    );
    // A Bootstrap .col-1 at xl: (1140 - 24 - 11 * 24) / 12 = 71 px of content.
    let columns = layout(&resolved, 1280.0);
    assert_eq!(columns.columns[0].width, 71.0);
}

#[test]
fn the_tailwind_preset_follows_the_container_widths() {
    let tailwind = ColumnSpec {
        preset: Some(Preset::Tailwind),
        ..ColumnSpec::default()
    };
    let at = |width: f64| resolve(&tailwind, width);
    assert_eq!(at(639.0).max_width, None);
    assert_eq!(at(640.0).max_width, Some(640.0));
    assert_eq!(at(1536.0).max_width, Some(1536.0));
    assert_eq!((at(1000.0).gutter, at(1000.0).margin), (24.0, 16.0));
}

#[test]
fn a_field_of_its_own_wins_over_the_preset() {
    let spec = ColumnSpec {
        preset: Some(Preset::Bootstrap),
        columns: Some(4),
        gutter: Some(16.0),
        max_width: Some(600.0),
        ..ColumnSpec::default()
    };
    let resolved = resolve(&spec, 1440.0);
    assert_eq!(
        (
            resolved.columns,
            resolved.gutter,
            resolved.margin,
            resolved.max_width
        ),
        (4, 16.0, 12.0, Some(600.0))
    );
}

#[test]
fn without_preset_and_fields_the_defaults_apply() {
    let resolved = resolve(&ColumnSpec::default(), 1024.0);
    assert_eq!(
        (
            resolved.columns,
            resolved.gutter,
            resolved.margin,
            resolved.max_width
        ),
        (DEFAULT_COLUMNS, DEFAULT_GUTTER, 0.0, None)
    );
    assert_eq!(resolved.align, Align::Center);
}

#[test]
fn a_breakpoint_takes_its_own_setup_else_the_one_for_any() {
    let setups = std::collections::BTreeMap::from([
        ("mobile".to_owned(), spec(4, 16.0, 16.0, None)),
        ("*".to_owned(), spec(12, 24.0, 12.0, None)),
    ]);
    assert_eq!(
        spec_for(&setups, Some("mobile")).and_then(|s| s.columns),
        Some(4)
    );
    assert_eq!(
        spec_for(&setups, Some("tablet")).and_then(|s| s.columns),
        Some(12)
    );
    assert_eq!(spec_for(&setups, None).and_then(|s| s.columns), Some(12));
    assert!(spec_for(&std::collections::BTreeMap::new(), Some("mobile")).is_none());
}

#[test]
fn widths_are_rounded_to_a_thousandth() {
    let layout = layout(&resolve(&spec(3, 0.0, 0.0, None), 100.0), 100.0);
    assert_eq!(layout.columns[1].x, 33.333);
}

#[test]
fn the_bootstrap_fixture_setups_give_the_e2e_numbers() {
    // tests/fixtures/grid-bootstrap.html and web/e2e/page.e2e.test.ts use exactly these.
    let mobile = layout(&resolve(&spec(4, 16.0, 8.0, None), 375.0), 375.0);
    assert_eq!(
        xs(&mobile),
        vec![
            (8.0, 77.75),
            (101.75, 77.75),
            (195.5, 77.75),
            (289.25, 77.75)
        ]
    );
    let desktop = layout(
        &resolve(&spec(12, 24.0, 12.0, Some(1200.0)), 1440.0),
        1440.0,
    );
    let columns = xs(&desktop);
    assert_eq!(columns[0], (132.0, 76.0));
    assert_eq!(columns[11], (1232.0, 76.0));
}

// RB-080-66: the panel shows a template's values as the placeholders of the empty fields, so it
// is clear what applies; they come from here, the one source.
#[test]
fn the_templates_name_their_values_for_the_panel() {
    let templates = templates();
    assert_eq!(
        templates["bootstrap"],
        serde_json::json!({ "columns": 12, "gutter": 24.0, "margin": 12.0,
                            "maxWidth": { "min": 540.0, "max": 1320.0 } })
    );
    assert_eq!(templates["tailwind"]["margin"], 16.0);
    assert_eq!(templates["tailwind"]["maxWidth"]["max"], 1536.0);
    assert_eq!(
        templates[""],
        serde_json::json!({ "columns": 12, "gutter": 24.0, "margin": 0.0, "maxWidth": null })
    );
    // The templates are what `resolve` uses.
    let bootstrap = resolve(
        &ColumnSpec {
            preset: Some(Preset::Bootstrap),
            ..ColumnSpec::default()
        },
        1400.0,
    );
    assert_eq!(bootstrap.margin, 12.0);
    assert_eq!(bootstrap.max_width, Some(1320.0));
}
