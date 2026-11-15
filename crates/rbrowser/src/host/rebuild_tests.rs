use rb_config::{EngineKind, Layout, LayoutCell, Zoom};

use super::*;
use crate::host::state::{default_config, layout_for};

/// Desktop, tablet and mobile in one row, the mobile one on Firefox (RB-080-38).
fn opened_state() -> (HostState, Layout) {
    let mut config = default_config();
    config.breakpoints[2].engine = EngineKind::Firefox;
    let layout = layout_for(&config);
    let mut state = HostState {
        config,
        ..HostState::default()
    };
    let rebuilt = rebuild_state(&mut state, &layout).expect("opens");
    assert_eq!(rebuilt.opened, [ViewId(1), ViewId(2), ViewId(3)]);
    (state, layout)
}

/// One row with `breakpoints` in that order, as a layout mode's grid (`ModeSpec::grid`).
fn row(breakpoints: &[&str]) -> Layout {
    Layout {
        columns: breakpoints.len() as u32,
        rows: 1,
        cells: breakpoints
            .iter()
            .enumerate()
            .map(|(index, breakpoint)| LayoutCell {
                breakpoint: (*breakpoint).to_owned(),
                col: index as u32 + 1,
                row: 1,
                col_span: 1,
                row_span: 1,
                zoom: Zoom::Fit,
            })
            .collect(),
        ..Layout::default()
    }
}

fn engine_of(state: &HostState, view: ViewId) -> EngineKind {
    state
        .views
        .get(&view)
        .map(|entry| state.info(entry).engine)
        .expect("the view exists")
}

/// A layout mode (grid, horizontal, split, canvas) and the way back keep every view, the
/// Firefox one included: its id, its engine and so its window (RB-080-38).
#[test]
fn a_mode_grid_and_the_way_back_keep_the_firefox_view() {
    let (mut state, grid) = opened_state();
    let mode = rebuild_state(&mut state, &row(&["mobile", "desktop", "tablet"])).expect("mode");
    assert!(mode.opened.is_empty(), "{:?}", mode.opened);
    assert!(mode.closed.is_empty(), "{:?}", mode.closed);
    assert_eq!(mode.order, [ViewId(3), ViewId(1), ViewId(2)]);
    assert_eq!(engine_of(&state, ViewId(3)), EngineKind::Firefox);
    let back = rebuild_state(&mut state, &grid).expect("grid");
    assert!(back.opened.is_empty() && back.closed.is_empty());
    assert_eq!(back.order, [ViewId(1), ViewId(2), ViewId(3)]);
    assert_eq!(engine_of(&state, ViewId(3)), EngineKind::Firefox);
    assert_eq!(state.next_id, 3, "no view id was spent");
}

/// A config reload with the same cells keeps the views; one that moves a breakpoint to another
/// engine keeps the view too (the external windows swap the window, the cell stays).
#[test]
fn a_config_reload_keeps_the_views_and_follows_a_changed_engine() {
    let (mut state, grid) = opened_state();
    let again = rebuild_state(&mut state, &grid).expect("reload");
    assert!(again.opened.is_empty() && again.closed.is_empty());
    state.config.breakpoints[1].engine = EngineKind::Webkit;
    let moved = rebuild_state(&mut state, &grid).expect("reload");
    assert!(moved.opened.is_empty() && moved.closed.is_empty());
    assert_eq!(engine_of(&state, ViewId(2)), EngineKind::Webkit);
    assert_eq!(engine_of(&state, ViewId(3)), EngineKind::Firefox);
}

/// Every label strip names its own cell's breakpoint and every badge its own view's engine,
/// with a Firefox view among Chromium ones and after a reorder (the "md md" finding of
/// RB-080-38 is not in the host's state).
#[test]
fn labels_and_engines_stay_with_their_cells() {
    let (mut state, _) = opened_state();
    let mode = rebuild_state(&mut state, &row(&["mobile", "desktop", "tablet"])).expect("mode");
    let labels: Vec<(ViewId, String)> = mode
        .cells
        .iter()
        .map(|cell| (cell.view, cell.label.clone()))
        .collect();
    assert_eq!(
        labels,
        [
            (ViewId(3), "mobile  393 \u{d7} 852".to_owned()),
            (ViewId(1), "desktop  1440 \u{d7} 900".to_owned()),
            (ViewId(2), "tablet  768 \u{d7} 1024".to_owned()),
        ]
    );
    let mut names: Vec<&str> = labels.iter().map(|(_, label)| label.as_str()).collect();
    names.dedup();
    assert_eq!(names.len(), 3, "no two cells carry the same label");
    for (view, label) in &labels {
        let expected = match state.views[view].cell_breakpoint.as_deref() {
            Some("mobile") => EngineKind::Firefox,
            _ => EngineKind::Chromium,
        };
        assert_eq!(engine_of(&state, *view), expected, "{label}");
    }
}

/// Desktop, tablet and mobile in explicit cells, the tablet hidden by the selection
/// (RB-080-41): its view is not opened, its cell keeps its slot as a `Hidden` leaf, and the
/// visible cells are numbered as the host's cell list has them.
#[test]
fn a_hidden_breakpoint_of_an_explicit_grid_keeps_an_empty_cell() {
    let mut config = default_config();
    config.layout.hidden = vec!["tablet".to_owned()];
    let layout = config.layout.clone();
    let mut state = HostState {
        config,
        ..HostState::default()
    };
    let rebuilt = rebuild_state(&mut state, &layout).expect("opens");
    assert_eq!(rebuilt.opened, [ViewId(1), ViewId(2)]);
    let shown: Vec<Option<String>> = rebuilt
        .order
        .iter()
        .map(|id| state.views[id].cell_breakpoint.clone())
        .collect();
    assert_eq!(
        shown,
        [Some("desktop".to_owned()), Some("mobile".to_owned())]
    );
    assert_eq!(rebuilt.cells.len(), 2);
    let crate::layout_plan::NodeKind::Split { children, .. } = &rebuilt.plan.kind else {
        panic!("{:?}", rebuilt.plan);
    };
    assert_eq!(
        children[1].kind,
        crate::layout_plan::NodeKind::Hidden("tablet".to_owned())
    );
    assert_eq!(children[2].kind, crate::layout_plan::NodeKind::Cell(1));
    assert_eq!(state.hidden, ["tablet"]);
    assert_eq!(state.layout.cells.len(), 3, "the full layout is kept");
    assert_eq!(state.visible_layout().cells.len(), 2);
    // Opening it again: the view comes back, the others stay.
    state.config.layout.hidden.clear();
    let again = rebuild_state(&mut state, &layout).expect("rebuilds");
    assert_eq!(again.opened, [ViewId(3)]);
    assert!(again.closed.is_empty());
    assert_eq!(again.order, [ViewId(1), ViewId(3), ViewId(2)]);
    // Hiding one closes only its view.
    state.config.layout.hidden = vec!["desktop".to_owned()];
    let fewer = rebuild_state(&mut state, &layout).expect("rebuilds");
    assert_eq!(fewer.closed, [ViewId(1)]);
    assert!(fewer.opened.is_empty());
}

/// A selection that would hide every breakpoint hides none: the grid never ends up empty.
#[test]
fn hiding_every_breakpoint_hides_none() {
    let mut config = default_config();
    config.layout.hidden = vec![
        "desktop".to_owned(),
        "tablet".to_owned(),
        "mobile".to_owned(),
    ];
    let layout = config.layout.clone();
    let mut state = HostState {
        config,
        ..HostState::default()
    };
    let rebuilt = rebuild_state(&mut state, &layout).expect("opens");
    assert_eq!(rebuilt.opened.len(), 3);
    assert!(state.hidden.is_empty());
}

/// The computed grid (no `layout.cells`) flows anew without the hidden breakpoint.
#[test]
fn the_computed_grid_flows_without_the_hidden_breakpoint() {
    let mut config = default_config();
    config.layout.cells.clear();
    config.layout.hidden = vec!["tablet".to_owned()];
    let layout = layout_for(&config);
    assert_eq!(layout.columns, 2);
    let cells: Vec<&str> = layout
        .cells
        .iter()
        .map(|cell| cell.breakpoint.as_str())
        .collect();
    assert_eq!(cells, ["desktop", "mobile"]);
    let mut state = HostState {
        config,
        ..HostState::default()
    };
    let rebuilt = rebuild_state(&mut state, &layout).expect("opens");
    assert_eq!(rebuilt.opened.len(), 2);
    assert!(
        !matches!(
            &rebuilt.plan.kind,
            crate::layout_plan::NodeKind::Split { children, .. }
                if children.iter().any(|child| matches!(child.kind, crate::layout_plan::NodeKind::Hidden(_)))
        ),
        "no empty cell in a computed grid"
    );
}
