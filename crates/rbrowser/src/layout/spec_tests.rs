use super::*;

fn grid() -> Layout {
    let cell = |breakpoint: &str, col: u32, row: u32| LayoutCell {
        breakpoint: breakpoint.to_owned(),
        col,
        row,
        col_span: 1,
        row_span: 1,
        zoom: Zoom::Fit,
    };
    Layout {
        columns: 2,
        rows: 2,
        gap: 8,
        cells: vec![
            cell("desktop", 2, 2),
            cell("mobile", 1, 1),
            cell("tablet", 2, 1),
        ],
        ..Default::default()
    }
}

fn known(id: &str) -> bool {
    ["mobile", "tablet", "desktop", "wide"].contains(&id)
}

fn spec_of(mode: LayoutMode, fields: ModeFields, active: Option<&str>) -> ModeSpec {
    let grid = grid();
    let settings = LayoutModeSettings::default();
    let session = ModeSession {
        fields,
        ..ModeSession::default()
    };
    resolve_mode(
        mode,
        &session,
        &ModeInputs {
            grid: &grid,
            settings: &settings,
            active,
            default_zoom: Zoom::Fit,
            known: &known,
            hidden: &[],
        },
    )
    .expect("a mode")
}

#[test]
fn grid_and_focus_are_no_mode() {
    let grid = grid();
    let settings = LayoutModeSettings::default();
    let inputs = ModeInputs {
        grid: &grid,
        settings: &settings,
        active: None,
        default_zoom: Zoom::Fit,
        known: &known,
        hidden: &[],
    };
    assert!(resolve_mode(LayoutMode::Grid, &ModeSession::default(), &inputs).is_none());
    assert!(resolve_mode(LayoutMode::Focus, &ModeSession::default(), &inputs).is_none());
}

#[test]
fn horizontal_shows_the_grid_in_reading_order_with_the_settings_height() {
    let spec = spec_of(LayoutMode::Horizontal, ModeFields::default(), None);
    assert_eq!(spec.breakpoints, ["mobile", "tablet", "desktop"]);
    assert_eq!(spec.mode(), LayoutMode::Horizontal);
    assert_eq!(
        spec.kind,
        ModeKind::Horizontal {
            height: StripHeight::Fit,
            zoom: Zoom::Fit,
            first: 0
        }
    );
    let fixed = spec_of(
        LayoutMode::Horizontal,
        ModeFields {
            height: Some(StripHeight::Px(500)),
            gap: Some(16),
            ..ModeFields::default()
        },
        None,
    );
    assert_eq!(fixed.gap, 16);
    assert!(matches!(
        fixed.kind,
        ModeKind::Horizontal {
            height: StripHeight::Px(500),
            ..
        }
    ));
}

#[test]
fn split_defaults_to_the_active_view_against_the_others() {
    let spec = spec_of(LayoutMode::Split, ModeFields::default(), Some("tablet"));
    assert_eq!(spec.breakpoints, ["tablet", "mobile", "desktop"]);
    assert_eq!(
        spec.kind,
        ModeKind::Split {
            orientation: SplitOrientation::Vertical,
            ratio: 0.5,
            primary: 1
        }
    );
    let without_active = spec_of(LayoutMode::Split, ModeFields::default(), None);
    assert_eq!(without_active.breakpoints[0], "mobile");
}

#[test]
fn split_takes_the_presets_sides_and_drops_unknown_and_repeated_ids() {
    let spec = spec_of(
        LayoutMode::Split,
        ModeFields {
            primary: vec!["desktop".into(), "nope".into()],
            secondary: vec!["desktop".into(), "mobile".into(), "mobile".into()],
            ratio: Some(0.7),
            orientation: Some(SplitOrientation::Horizontal),
            ..ModeFields::default()
        },
        None,
    );
    assert_eq!(spec.breakpoints, ["desktop", "mobile"]);
    assert_eq!(
        spec.kind,
        ModeKind::Split {
            orientation: SplitOrientation::Horizontal,
            ratio: 0.7,
            primary: 1
        }
    );
}

#[test]
fn canvas_adds_the_items_breakpoints_after_the_grids() {
    let item = |breakpoint: &str| CanvasItem {
        breakpoint: breakpoint.into(),
        x: 0,
        y: 0,
        zoom: 0.5,
    };
    let spec = spec_of(
        LayoutMode::Canvas,
        ModeFields {
            items: vec![item("wide"), item("mobile"), item("ghost")],
            ..ModeFields::default()
        },
        None,
    );
    assert_eq!(spec.breakpoints, ["mobile", "tablet", "desktop", "wide"]);
    assert!(matches!(
        spec.kind,
        ModeKind::Canvas {
            snap: 8,
            camera: None,
            partial: true,
            ..
        }
    ));
}

/// The views of every mode are the same breakpoints, so a mode switch keeps the browsers.
#[test]
fn every_mode_shows_the_grids_breakpoints() {
    let mut sets: Vec<Vec<String>> = [
        LayoutMode::Horizontal,
        LayoutMode::Split,
        LayoutMode::Canvas,
    ]
    .into_iter()
    .map(|mode| {
        let mut ids = spec_of(mode, ModeFields::default(), None).breakpoints;
        ids.sort();
        ids
    })
    .collect();
    sets.dedup();
    assert_eq!(sets.len(), 1, "{sets:?}");
}

#[test]
fn the_rebuild_grid_is_one_row_in_order() {
    let spec = spec_of(LayoutMode::Split, ModeFields::default(), Some("desktop"));
    let grid = spec.grid();
    assert_eq!((grid.columns, grid.rows, grid.gap), (3, 1, 8));
    let order: Vec<(&str, u32)> = grid
        .cells
        .iter()
        .map(|cell| (cell.breakpoint.as_str(), cell.col))
        .collect();
    assert_eq!(order, [("desktop", 1), ("mobile", 2), ("tablet", 3)]);
}

#[test]
fn entering_keeps_the_session_unless_preset_mode_or_file_changed() {
    let stored = ModeFields {
        ratio: Some(0.5),
        ..ModeFields::default()
    };
    let mut session = ModeSession::default();
    session.enter(Some("pair"), LayoutMode::Split, &stored);
    session.fields.ratio = Some(0.65);
    session.minimap = Some(false);
    session.enter(Some("pair"), LayoutMode::Split, &stored);
    assert_eq!(session.fields.ratio, Some(0.65), "ahead of the file, kept");

    let edited = ModeFields {
        ratio: Some(0.3),
        ..ModeFields::default()
    };
    session.enter(Some("pair"), LayoutMode::Split, &edited);
    assert_eq!(session.fields.ratio, Some(0.3), "an edit by hand wins");

    session.first = 3;
    session.enter(Some("pair"), LayoutMode::Horizontal, &ModeFields::default());
    assert_eq!(session.first, 0, "a new mode starts afresh");
    assert_eq!(session.minimap, Some(false), "the minimap switch stays");
}

#[test]
fn arrange_dispatches_to_the_mode() {
    let sizes: Vec<Item> = ["mobile", "tablet", "desktop"]
        .iter()
        .map(|id| Item {
            breakpoint: (*id).to_owned(),
            width: 400,
            height: 800,
        })
        .collect();
    let area = CellRect {
        x: 0,
        y: 44,
        width: 1280,
        height: 700,
    };
    for mode in [
        LayoutMode::Horizontal,
        LayoutMode::Split,
        LayoutMode::Canvas,
    ] {
        let slots = spec_of(mode, ModeFields::default(), None).arrange(&sizes, area, 20);
        assert_eq!(slots.len(), 3, "{mode:?}");
        assert!(slots.iter().all(|slot| slot.visible), "{mode:?}: {slots:?}");
    }
}

/// A hidden breakpoint (RB-080-41) gets no view in any mode: not in the strip's order, not on
/// a split's side it is named for, not on the canvas even with an item of its own.
#[test]
fn a_hidden_breakpoint_is_left_out_of_every_mode() {
    let grid = grid();
    let settings = LayoutModeSettings::default();
    let hidden = ["tablet".to_owned()];
    let inputs = ModeInputs {
        grid: &grid,
        settings: &settings,
        active: Some("tablet"),
        default_zoom: Zoom::Fit,
        known: &known,
        hidden: &hidden,
    };
    let strip =
        resolve_mode(LayoutMode::Horizontal, &ModeSession::default(), &inputs).expect("a mode");
    assert!(
        !strip.breakpoints.iter().any(|id| id == "tablet"),
        "{strip:?}"
    );
    let mut session = ModeSession::default();
    session.fields.primary = vec!["tablet".to_owned()];
    let split = resolve_mode(LayoutMode::Split, &session, &inputs).expect("a mode");
    assert!(
        !split.breakpoints.iter().any(|id| id == "tablet"),
        "{split:?}"
    );
    assert!(
        matches!(split.kind, ModeKind::Split { primary: 1, .. }),
        "{split:?}"
    );
    let mut session = ModeSession::default();
    session.fields.items = vec![CanvasItem {
        breakpoint: "tablet".to_owned(),
        x: 0,
        y: 0,
        zoom: 0.5,
    }];
    let canvas = resolve_mode(LayoutMode::Canvas, &session, &inputs).expect("a mode");
    assert!(
        !canvas.breakpoints.iter().any(|id| id == "tablet"),
        "{canvas:?}"
    );
    assert!(
        matches!(&canvas.kind, ModeKind::Canvas { items, .. } if items.len() == 1),
        "the item stays in the fields"
    );
}
