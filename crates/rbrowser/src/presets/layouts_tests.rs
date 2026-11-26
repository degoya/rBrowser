use rb_config::{LayoutCell, LayoutPreset, Layouts};

use super::*;
use crate::host::state::default_config;

/// desktop, tablet, mobile (the default config) plus two presets: `wide` (a 2x1 grid of
/// desktop and mobile at 0.5) and `focus` (focus mode on tablet).
fn config() -> ProjectConfig {
    let mut config = default_config();
    config.layout = Layout::default();
    let cell = |breakpoint: &str, col: u32, zoom: Zoom| LayoutCell {
        breakpoint: breakpoint.to_owned(),
        col,
        row: 1,
        col_span: 1,
        row_span: 1,
        zoom,
    };
    let mut layouts = Layouts {
        default: Some("wide".to_owned()),
        ..Layouts::default()
    };
    layouts.presets.insert(
        "wide".to_owned(),
        LayoutPreset {
            mode: LayoutMode::Grid,
            layout: Some(Layout {
                columns: 2,
                rows: 1,
                gap: 4,
                cells: vec![
                    cell("desktop", 1, Zoom::Factor(0.5)),
                    cell("mobile", 2, Zoom::Fit),
                ],
                ..Default::default()
            }),
            focus: None,
            ..LayoutPreset::default()
        },
    );
    layouts.presets.insert(
        "focus".to_owned(),
        LayoutPreset {
            mode: LayoutMode::Focus,
            layout: None,
            focus: Some("tablet".to_owned()),
            ..LayoutPreset::default()
        },
    );
    config.layouts = layouts;
    config
}

fn breakpoints(layout: &Layout) -> Vec<&str> {
    layout
        .cells
        .iter()
        .map(|cell| cell.breakpoint.as_str())
        .collect()
}

#[test]
fn without_a_preset_the_grid_is_one_row_of_breakpoints_at_the_default_zoom() {
    let (layout, selection) =
        resolve(&config(), &Selection::default(), Zoom::Factor(0.5)).expect("resolves");
    assert_eq!(breakpoints(&layout), ["desktop", "tablet", "mobile"]);
    assert!(
        layout
            .cells
            .iter()
            .all(|cell| cell.zoom == Zoom::Factor(0.5))
    );
    assert_eq!(selection, Selection::default());
}

#[test]
fn cells_written_in_the_config_keep_their_own_zoom() {
    let mut config = config();
    config.layout = layout_for(&config);
    config.layout.cells[0].zoom = Zoom::Factor(0.75);
    let (layout, _) = resolve(&config, &Selection::default(), Zoom::Factor(0.5)).expect("resolves");
    assert_eq!(layout.cells[0].zoom, Zoom::Factor(0.75));
    assert_eq!(layout.cells[1].zoom, Zoom::Fit);
}

#[test]
fn a_grid_preset_brings_its_own_layout() {
    let selection = Selection::preset(&config(), "wide").expect("known");
    let (layout, selection) = resolve(&config(), &selection, Zoom::Fit).expect("resolves");
    assert_eq!(breakpoints(&layout), ["desktop", "mobile"]);
    assert_eq!(layout.cells[0].zoom, Zoom::Factor(0.5));
    assert_eq!(selection.mode, LayoutMode::Grid);
    assert_eq!(selection.focus, None);
}

#[test]
fn a_focus_preset_shows_one_view_of_its_breakpoint() {
    let selection = Selection::preset(&config(), "focus").expect("known");
    let (layout, selection) = resolve(&config(), &selection, Zoom::Fit).expect("resolves");
    assert_eq!((layout.columns, layout.rows), (1, 1));
    assert_eq!(breakpoints(&layout), ["tablet"]);
    assert_eq!(selection.mode, LayoutMode::Focus);
    assert_eq!(selection.focus.as_deref(), Some("tablet"));
}

#[test]
fn focus_mode_without_a_breakpoint_takes_the_first() {
    let selection = Selection {
        preset: None,
        mode: LayoutMode::Focus,
        focus: Some("gone".to_owned()),
    };
    let (layout, selection) = resolve(&config(), &selection, Zoom::Fit).expect("resolves");
    assert_eq!(breakpoints(&layout), ["desktop"]);
    assert_eq!(selection.focus.as_deref(), Some("desktop"));
}

#[test]
fn an_unknown_preset_is_refused() {
    assert_eq!(
        Selection::preset(&config(), "nope"),
        Err(PresetError::UnknownPreset("nope".to_owned()))
    );
}

#[test]
fn presets_are_numbered_in_config_order() {
    assert_eq!(preset_at(&config(), 1), Some("wide"));
    assert_eq!(preset_at(&config(), 2), Some("focus"));
    assert_eq!(preset_at(&config(), 3), None);
    assert_eq!(preset_at(&config(), 0), None);
}

#[test]
fn the_start_selection_is_the_default_preset_when_it_exists() {
    assert_eq!(start_selection(&config()).preset.as_deref(), Some("wide"));
    let mut config = config();
    config.layouts.default = Some("gone".to_owned());
    assert_eq!(start_selection(&config), Selection::default());
}

#[test]
fn focus_mode_toggles_on_at_the_active_breakpoint_and_back_to_the_grid_preset() {
    let config = config();
    let wide = Selection::preset(&config, "wide").expect("known");
    let on = wide.toggled_focus(&config, Some("mobile"));
    assert_eq!(on.mode, LayoutMode::Focus);
    assert_eq!(on.preset.as_deref(), Some("wide"));
    assert_eq!(on.focus.as_deref(), Some("mobile"));
    assert_eq!(on.toggled_focus(&config, None), wide);
    // Leaving a focus preset falls back to the top-level layout in the built-in mode, the
    // canvas (RB-099-26).
    let focus = Selection::preset(&config, "focus").expect("known");
    let back = focus.toggled_focus(&config, None);
    assert_eq!(back, Selection::default());
    assert_eq!((back.preset, back.mode), (None, LayoutMode::Canvas));
}

#[test]
fn the_focus_moves_through_the_breakpoints_and_wraps() {
    let config = config();
    let at = |focus: &str| Selection {
        preset: None,
        mode: LayoutMode::Focus,
        focus: Some(focus.to_owned()),
    };
    let next = at("desktop").stepped(&config, true).expect("focus mode");
    assert_eq!(next.focus.as_deref(), Some("tablet"));
    let wrapped = at("mobile").stepped(&config, true).expect("focus mode");
    assert_eq!(wrapped.focus.as_deref(), Some("desktop"));
    let back = at("desktop").stepped(&config, false).expect("focus mode");
    assert_eq!(back.focus.as_deref(), Some("mobile"));
    assert_eq!(
        Selection::default().stepped(&config, true),
        Err(PresetError::NotInFocusMode)
    );
}

#[test]
fn zoom_to_fit_fits_every_cell() {
    let selection = Selection::preset(&config(), "wide").expect("known");
    let (layout, _) = resolve(&config(), &selection, Zoom::Fit).expect("resolves");
    assert!(
        fitted(&layout)
            .cells
            .iter()
            .all(|cell| cell.zoom == Zoom::Fit)
    );
}

#[test]
fn a_config_without_breakpoints_has_no_focus_mode() {
    let mut config = config();
    config.breakpoints.clear();
    config.layouts = Layouts::default();
    let selection = Selection {
        preset: None,
        mode: LayoutMode::Focus,
        focus: None,
    };
    assert_eq!(
        resolve(&config, &selection, Zoom::Fit),
        Err(PresetError::NoBreakpoints)
    );
}

/// A hidden breakpoint (RB-080-41) is not in focus mode's order: the arrow keys skip it, and a
/// focus on it falls to the first open one.
#[test]
fn focus_mode_skips_the_hidden_breakpoints() {
    let mut config = config();
    config.layout.hidden = vec!["tablet".to_owned()];
    let selection = Selection {
        preset: None,
        mode: LayoutMode::Focus,
        focus: Some("desktop".to_owned()),
    };
    let next = selection.stepped(&config, true).expect("steps");
    assert_eq!(next.focus.as_deref(), Some("mobile"));
    let (_, resolved) = resolve(
        &config,
        &Selection {
            focus: Some("tablet".to_owned()),
            ..selection
        },
        Zoom::Fit,
    )
    .expect("resolves");
    assert_eq!(resolved.focus.as_deref(), Some("desktop"));
}
