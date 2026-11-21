use std::sync::Arc;

use rb_config::{Layout, LayoutCell, ProjectConfig, StripHeight};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::host::nav::marker_url;
use crate::host::ui::recording::{RecordingUi, UiCall};
use crate::layout::spec::ModeKind;

const FIXTURE: &str = include_str!("../../../../tests/fixtures/grid-project/.rbrowser/config.json");

fn setup() -> (Arc<RecordingUi>, Arc<Host>) {
    let config: ProjectConfig = serde_json::from_str(FIXTURE).expect("fixture parses");
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config, None).expect("opens");
    for id in 1..=20 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    (ui, host)
}

fn strip(breakpoints: &[&str]) -> ModeSpec {
    ModeSpec {
        breakpoints: breakpoints.iter().map(|id| (*id).to_owned()).collect(),
        gap: 8,
        kind: ModeKind::Horizontal {
            height: StripHeight::Fit,
            zoom: Zoom::Factor(1.0),
            first: 0,
        },
    }
}

const ALL: [&str; 6] = [
    "desktop",
    "laptop",
    "tablet",
    "tablet-landscape",
    "mobile",
    "mobile-small",
];

fn area(width: u32) -> CellRect {
    CellRect {
        x: 0,
        y: 44,
        width,
        height: 700,
    }
}

fn last_places(ui: &RecordingUi) -> Vec<PlacedView> {
    ui.calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            UiCall::Place(placed) => Some(placed),
            UiCall::Grid(grid) => grid.placed,
            _ => None,
        })
        .expect("views were placed")
}

/// Grid -> horizontal -> grid: the same six views throughout, no view opened or closed.
#[tokio::test]
async fn a_mode_switch_keeps_every_view() {
    let (ui, host) = setup();
    let before: Vec<ViewId> = host.state().order.clone();
    host.on_grid_area(area(2000));
    let order = host.set_mode(strip(&ALL)).expect("horizontal");
    let mut sorted = order.clone();
    sorted.sort();
    let mut expected = before.clone();
    expected.sort();
    assert_eq!(sorted, expected, "the browsers live on");
    let grid = ui.last_grid().expect("a grid");
    assert!(grid.placed.is_some(), "a mode places its views");
    assert_eq!(
        host.mode_spec().map(|spec| spec.mode()),
        Some(rb_config::LayoutMode::Horizontal)
    );

    let layout = crate::host::state::layout_for(&host.config());
    host.rebuild(&layout).expect("back to the grid");
    assert!(host.mode_spec().is_none());
    assert!(ui.last_grid().expect("a grid").placed.is_none());
    let mut back = host.state().order.clone();
    back.sort();
    assert_eq!(back, expected);
}

/// At 1600 DIP the strip at scale 1 shows desktop only; the other views are off screen and
/// reported hidden, and scrolling brings the next ones in.
#[tokio::test]
async fn views_out_of_sight_are_marked_and_scrolling_places_them_again() {
    let (ui, host) = setup();
    host.set_mode(strip(&ALL)).expect("horizontal");
    host.on_grid_area(area(1600));
    let placed = last_places(&ui);
    assert_eq!(placed.len(), 6);
    let shown: Vec<bool> = placed.iter().map(|place| place.visible).collect();
    assert_eq!(shown, [true, false, false, false, false, false]);
    assert_eq!(placed[0].outer.x, 8);
    let state = host.state();
    let off: Vec<bool> = state
        .order
        .iter()
        .map(|id| state.views[id].off_screen)
        .collect();
    let order = state.order.clone();
    drop(state);
    assert_eq!(off, [false, true, true, true, true, true]);
    // What the flow log names (RB-090-84).
    let asked: Vec<bool> = order.iter().map(|id| host.is_off_screen(*id)).collect();
    assert_eq!(asked, off);
    assert!(!host.is_off_screen(ViewId(999)));

    assert!(host.update_mode(|spec| {
        if let ModeKind::Horizontal { first, .. } = &mut spec.kind {
            *first = 4;
        }
    }));
    let placed = last_places(&ui);
    let shown: Vec<bool> = placed.iter().map(|place| place.visible).collect();
    assert_eq!(shown, [false, false, false, false, true, true]);
    assert_eq!(placed[4].outer.x, 8);
}

#[tokio::test]
async fn the_cells_follow_the_places() {
    let (_ui, host) = setup();
    host.on_grid_area(area(1600));
    host.set_mode(strip(&["mobile", "tablet"]))
        .expect("horizontal");
    let state = host.state();
    assert_eq!(state.order.len(), 2, "views not in the mode close");
    assert_eq!(state.cells.len(), 2);
    assert_eq!(state.cells[0].breakpoint, "mobile");
    assert_eq!(state.cells[0].rect.width, 393);
    assert_eq!(state.cells[1].label.x, 8 + 393 + 8);
}

#[tokio::test]
async fn without_a_mode_nothing_is_placed() {
    let (ui, host) = setup();
    assert!(!host.update_mode(|_| {}));
    host.on_grid_area(area(1600));
    assert!(
        !ui.calls()
            .iter()
            .any(|call| matches!(call, UiCall::Place(_)))
    );
}

/// RB-090-89 (diag29): focus mode - a grid of one cell - after a mode that placed views off
/// screen shows the view it keeps; its off-screen mark stayed, and a played recording skipped the
/// view the person had just recorded in. The grid shows every view.
#[tokio::test]
async fn leaving_a_mode_shows_the_views_it_had_placed_off_screen() {
    let (_ui, host) = setup();
    host.set_mode(strip(&ALL)).expect("horizontal");
    host.on_grid_area(area(1600));
    let tablet = host.state().order[2];
    assert!(host.is_off_screen(tablet), "out of the strip's sight");

    let focus = Layout {
        columns: 1,
        rows: 1,
        gap: 8,
        cells: vec![LayoutCell {
            breakpoint: "tablet".to_owned(),
            col: 1,
            row: 1,
            col_span: 1,
            row_span: 1,
            zoom: Zoom::Fit,
        }],
        ..Default::default()
    };
    let kept = host.rebuild(&focus).expect("focus mode");
    assert_eq!(kept, [tablet], "the view lives on");
    assert!(!host.is_off_screen(tablet), "focus mode shows it");

    host.set_mode(strip(&ALL)).expect("horizontal again");
    assert!(host.state().views.values().any(|entry| entry.off_screen));
    let grid = crate::host::state::layout_for(&host.config());
    host.rebuild(&grid).expect("the grid");
    let state = host.state();
    assert!(
        state.order.iter().all(|id| !state.views[id].off_screen),
        "the grid hides no view"
    );
}
