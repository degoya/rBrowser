use super::*;

fn existing(cells: &[&str]) -> Vec<(ViewId, Option<String>)> {
    cells
        .iter()
        .enumerate()
        .map(|(index, cell)| (ViewId(index as u32 + 1), Some((*cell).to_owned())))
        .collect()
}

#[test]
fn a_changed_column_count_keeps_every_view() {
    let views = existing(&["desktop", "tablet", "mobile"]);
    assert_eq!(
        match_views(&views, &["desktop", "tablet", "mobile"]),
        vec![Some(ViewId(1)), Some(ViewId(2)), Some(ViewId(3))]
    );
}

#[test]
fn reordered_cells_keep_their_views_and_new_breakpoints_get_new_ones() {
    let views = existing(&["desktop", "tablet", "mobile", "mobile"]);
    assert_eq!(
        match_views(&views, &["mobile", "laptop", "desktop", "mobile", "mobile"]),
        vec![
            Some(ViewId(3)),
            None,
            Some(ViewId(1)),
            Some(ViewId(4)),
            None
        ]
    );
}

#[test]
fn a_breakpoint_without_emulation_keeps_the_display_dpr() {
    let config = default_config();
    let desktop = &config.breakpoints[0];
    let emulation = emulation_for(desktop, &EmulationFields::default());
    assert_eq!((emulation.width, emulation.height), (1440, 900));
    assert_eq!(emulation.dpr, 0.0);
    assert!(!emulation.mobile);
    let mobile = emulation_for(&config.breakpoints[2], &EmulationFields::default());
    assert!(
        mobile.touch && mobile.mobile,
        "touch implies mobile by default"
    );
}

#[test]
fn project_defaults_fill_what_the_breakpoint_leaves_out() {
    let config = default_config();
    let defaults = EmulationFields {
        dpr: Some(2.0),
        mobile: Some(false),
        ..EmulationFields::default()
    };
    let emulation = emulation_for(&config.breakpoints[2], &defaults);
    assert_eq!(emulation.dpr, 2.0);
    assert!(emulation.touch && !emulation.mobile);
}

#[test]
fn a_config_without_cells_gets_one_row_of_its_breakpoints() {
    let mut config = default_config();
    config.layout.cells.clear();
    let layout = layout_for(&config);
    assert_eq!((layout.columns, layout.rows), (3, 1));
    let ids: Vec<&str> = layout
        .cells
        .iter()
        .map(|cell| cell.breakpoint.as_str())
        .collect();
    assert_eq!(ids, vec!["desktop", "tablet", "mobile"]);
}

#[test]
fn an_extra_column_spans_every_row() {
    let config = default_config();
    let mut layout = config.layout.clone();
    layout.rows = 2;
    let extended = layout_with_extra_column(&layout, "mobile");
    assert_eq!(extended.columns, 4);
    let added = extended.cells.last().expect("added");
    assert_eq!((added.col, added.row, added.row_span), (4, 1, 2));
    rb_engine::compute_grid(
        &extended,
        CellRect {
            x: 0,
            y: 0,
            width: 1600,
            height: 900,
        },
        20,
    )
    .expect("still a valid grid");
}

#[test]
fn view_info_reports_the_applied_size_and_scale() {
    let config = default_config();
    let mut state = HostState {
        config,
        ..HostState::default()
    };
    let id = state.allocate_id();
    let desktop = state.config.breakpoints[0].clone();
    let mut view = ViewState::new(
        id,
        &desktop,
        emulation_for(&desktop, &EmulationFields::default()),
    );
    view.measured = Some(CellRect {
        x: 8,
        y: 72,
        width: 480,
        height: 300,
    });
    let info = state.info(&view);
    assert_eq!((info.width, info.height), (1440, 900));
    assert!((info.scale - 1.0 / 3.0).abs() < 1e-9);
    assert_eq!(info.label, "desktop");
    assert_eq!(info.session, "default");
    view.breakpoint = None;
    view.emulation.width = 500;
    view.emulation.height = 800;
    assert_eq!(state.info(&view).label, "500x800");
}
