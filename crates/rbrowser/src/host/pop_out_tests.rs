use std::sync::Arc;

use rb_config::{ProjectConfig, Zoom};
use rb_engine::{CellRect, ViewHost, ViewId};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::host::nav::marker_url;
use crate::host::ui::recording::{RecordingUi, UiCall};

const FIXTURE: &str = include_str!("../../../../tests/fixtures/grid-project/.rbrowser/config.json");

fn config() -> ProjectConfig {
    serde_json::from_str(FIXTURE).expect("fixture parses")
}

fn setup() -> (Arc<RecordingUi>, Arc<Host>) {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config(), None).expect("opens");
    for id in 1..=20 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    (ui, host)
}

fn options() -> PopOutOptions {
    PopOutOptions {
        keep_placeholder: true,
        fit_to_screen: true,
        remembered: None,
    }
}

/// The calls after the `skip` first.
fn calls_after(ui: &RecordingUi, skip: usize) -> Vec<UiCall> {
    ui.calls().into_iter().skip(skip).collect()
}

const DESKTOP: ViewId = ViewId(1);

#[tokio::test]
async fn popping_out_opens_the_window_at_the_breakpoint_and_leaves_a_placeholder() {
    let (ui, host) = setup();
    let before = ui.calls().len();
    assert!(host.pop_out(DESKTOP, options()).expect("pops out"));
    assert!(host.is_detached(DESKTOP));
    let calls = calls_after(&ui, before);
    let window = calls
        .iter()
        .find_map(|call| match call {
            UiCall::PopOut(window) => Some(window.clone()),
            _ => None,
        })
        .expect("the window opens");
    assert_eq!(window.view, DESKTOP);
    assert_eq!(window.label, "Desktop");
    assert_eq!((window.width, window.height), (1440, 900));
    assert!(window.fit_to_screen);
    // The grid is built again: the view's cell shows the placeholder, every view is kept.
    let grid = ui.last_grid().expect("a grid");
    assert_eq!(
        grid.detached,
        vec![DetachedCell {
            view: DESKTOP,
            placeholder: true
        }]
    );
    assert_eq!(grid.cells.len(), 6);
    assert_eq!(grid.cells[0].view, DESKTOP);
    // The window comes first, so the grid never takes the view back into its cell.
    let window_at = calls
        .iter()
        .position(|call| matches!(call, UiCall::PopOut(_)))
        .expect("window");
    let grid_at = calls
        .iter()
        .position(|call| matches!(call, UiCall::Grid(_)))
        .expect("grid");
    assert!(window_at < grid_at);
    // MCP and the shell see the same view, its id unchanged.
    assert_eq!(host.views().len(), 6);
    assert!(host.view(DESKTOP).is_some());
}

#[tokio::test]
async fn a_view_already_out_comes_to_the_front_instead() {
    let (ui, host) = setup();
    host.pop_out(DESKTOP, options()).expect("pops out");
    let before = ui.calls().len();
    assert!(!host.pop_out(DESKTOP, options()).expect("answers"));
    assert_eq!(calls_after(&ui, before), vec![UiCall::RaisePopOut(DESKTOP)]);
}

#[tokio::test]
async fn popping_in_closes_the_window_and_gives_the_cell_back() {
    let (ui, host) = setup();
    host.pop_out(DESKTOP, options()).expect("pops out");
    let before = ui.calls().len();
    assert!(host.pop_in(DESKTOP).expect("pops in"));
    assert!(!host.is_detached(DESKTOP));
    let calls = calls_after(&ui, before);
    let closed = calls
        .iter()
        .position(|call| *call == UiCall::PopIn(DESKTOP))
        .expect("the window closes");
    let grid = calls
        .iter()
        .position(|call| matches!(call, UiCall::Grid(grid) if grid.detached.is_empty()))
        .expect("the grid takes the view back");
    assert!(
        closed < grid,
        "the window lets the view go before the grid takes it"
    );
    // Twice is nothing.
    let again = ui.calls().len();
    assert!(!host.pop_in(DESKTOP).expect("answers"));
    assert!(calls_after(&ui, again).is_empty());
}

#[tokio::test]
async fn without_a_placeholder_the_cell_closes_its_gap() {
    let (ui, host) = setup();
    let options = PopOutOptions {
        keep_placeholder: false,
        ..options()
    };
    host.pop_out(ViewId(3), options).expect("pops out");
    let grid = ui.last_grid().expect("a grid");
    assert_eq!(
        grid.detached,
        vec![DetachedCell {
            view: ViewId(3),
            placeholder: false
        }]
    );
}

#[tokio::test]
async fn the_page_in_the_window_is_at_zoom_one_and_back_in_the_grid_at_its_cell_zoom() {
    let (_ui, host) = setup();
    // Tablet landscape (1024 wide) sits in a cell at zoom 0.25.
    let view = ViewId(4);
    host.on_grid_area(CellRect {
        x: 0,
        y: 44,
        width: 1600,
        height: 900,
    });
    let in_grid = host.view(view).expect("view").scale;
    assert!((in_grid - 0.25).abs() < 1e-9, "{in_grid}");
    host.pop_out(view, options()).expect("pops out");
    // Before the window reports its client area: the real size.
    assert!((host.view(view).expect("view").scale - 1.0).abs() < f64::EPSILON);
    // The window reports its client area, the size of the breakpoint.
    host.on_view_bounds(
        view,
        CellRect {
            x: 0,
            y: 0,
            width: 1024,
            height: 740,
        },
    );
    let info = host.view(view).expect("view");
    assert!((info.scale - 1.0).abs() < f64::EPSILON);
    assert_eq!(info.width, 1024);
    assert_eq!(info.height, 740);
    // A zoom step on the view does not reach the window...
    host.set_active_view(view).expect("active");
    host.zoom_active(false).expect("zooms");
    assert!((host.view(view).expect("view").scale - 1.0).abs() < f64::EPSILON);
    // ...and holds again once it is back in its cell.
    host.pop_in(view).expect("pops in");
    let back = host.view(view).expect("view").scale;
    assert!(back < 1.0, "{back}");
    assert!(matches!(host.state().views[&view].zoom, Zoom::Factor(_)));
}

#[tokio::test]
async fn unknown_views_are_refused() {
    let (_ui, host) = setup();
    assert!(host.pop_out(ViewId(99), options()).is_err());
    assert!(host.pop_in(ViewId(99)).is_err());
}

#[tokio::test]
async fn a_config_reload_keeps_the_window_and_a_dropped_view_ends_it() {
    let (ui, host) = setup();
    host.pop_out(DESKTOP, options()).expect("pops out");
    // The same config again: the view keeps its breakpoint and stays in its window.
    host.open_config(config(), None).expect("reopens");
    assert!(host.is_detached(DESKTOP));
    assert_eq!(
        ui.last_grid().expect("grid").detached,
        vec![DetachedCell {
            view: DESKTOP,
            placeholder: true
        }]
    );
    // Without the desktop breakpoint the view closes; nothing is detached any more (the window
    // closes with the view, `cef_app/grid.rs`).
    let mut smaller = config();
    smaller.layout.cells.remove(0);
    host.open_config(smaller, None).expect("reopens");
    assert!(!host.is_detached(DESKTOP));
    assert!(ui.last_grid().expect("grid").detached.is_empty());
}

#[tokio::test]
async fn a_new_start_has_every_view_in_the_grid() {
    // Nothing of a pop-out is kept across a restart: a new host starts with none.
    let (ui, host) = setup();
    let views: Vec<ViewId> = host.state().views.keys().copied().collect();
    assert_eq!(views.len(), 6);
    assert!(views.iter().all(|view| !host.is_detached(*view)));
    assert!(ui.last_grid().expect("grid").detached.is_empty());
}
