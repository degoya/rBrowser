//! A footer in a narrow view (RB-080-53): two lines when one does not hold the text, the
//! height of both taken from the view, the text in the middle.

use rb_config::{FooterAlign, FooterWrap, ViewFooterSettings};
use rb_engine::{CellRect, ViewHost as _, ViewId};

use super::super::Host;
use super::super::state::layout_for;
use super::super::ui::recording::{RecordingUi, UiCall};
use super::tests::{Setup, area, classic, config, footer_calls, setup};
use crate::view_footer::{CellFooter, FOOTER_HEIGHT};

/// A grid area so narrow that each of the three cells is about 200 DIP wide.
fn narrow() -> CellRect {
    CellRect {
        width: 640,
        ..area()
    }
}

fn sent(host: &Host, view: ViewId) -> CellFooter {
    host.state().footer.sent[&view].clone()
}

fn rect(setup: &Setup, index: usize) -> CellRect {
    setup.host.views()[index].cell.clone().expect("cell").rect
}

/// Lets the footer task follow the new area (or fails the test after a second).
async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

#[tokio::test]
async fn a_narrow_view_gets_a_footer_of_two_lines_and_gives_it_their_height() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(narrow());
    let xs = setup.host.views()[0].id;
    until(|| sent(&setup.host, xs).lines.len() == 2).await;
    let footer = sent(&setup.host, xs);
    assert!(rect(&setup, 0).width < 260, "{:?}", rect(&setup, 0));
    assert_eq!(
        footer.lines,
        ["xs | 0-767px | 360 \u{d7} 640", "max-width 767px"]
    );
    assert_eq!(footer.lines.join(" | "), footer.text);
    assert_eq!(footer.height(), 2 * FOOTER_HEIGHT);
    assert_eq!(setup.host.footer_height(xs), 2 * FOOTER_HEIGHT);
    // The view gives both lines; the cell's bottom stays where it was.
    let without = super::tests::setup(None);
    without.host.on_grid_area(narrow());
    let (with, bare) = (rect(&setup, 0), rect(&without, 0));
    assert_eq!(with.y, bare.y);
    assert_eq!(with.height + 2 * FOOTER_HEIGHT, bare.height);
}

#[tokio::test]
async fn the_footers_of_a_row_end_flush_whatever_their_lines() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(narrow());
    let views = setup.host.views();
    until(|| sent(&setup.host, views[0].id).lines.len() == 2).await;
    let bottoms: Vec<u32> = (0..views.len())
        .map(|index| {
            let rect = rect(&setup, index);
            rect.y.unsigned_abs() + rect.height + setup.host.footer_height(views[index].id)
        })
        .collect();
    assert!(
        bottoms.windows(2).all(|pair| pair[0] == pair[1]),
        "{bottoms:?}"
    );
}

#[tokio::test]
async fn what_two_lines_do_not_hold_is_in_the_tooltip() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(narrow());
    let md = setup.host.views()[1].id;
    until(|| sent(&setup.host, md).lines.len() == 2).await;
    let footer = sent(&setup.host, md);
    // 66 characters: more than two lines of a 200 DIP strip hold.
    assert!(
        footer.tooltip.starts_with(&footer.text),
        "{}",
        footer.tooltip
    );
    assert!(footer.tooltip.contains("min-width 768px"));
}

#[tokio::test]
async fn a_wide_view_keeps_one_line_in_the_middle() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    for view in setup.host.views() {
        let footer = sent(&setup.host, view.id);
        assert_eq!(footer.lines, std::slice::from_ref(&footer.text));
        assert!(footer.centered);
        assert_eq!(setup.host.footer_height(view.id), FOOTER_HEIGHT);
        // Nothing cut: the tooltip names the queries only.
        assert!(!footer.tooltip.contains(" | "), "{}", footer.tooltip);
    }
}

#[tokio::test]
async fn wrap_never_and_align_start_hold_without_a_restart() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(narrow());
    let xs = setup.host.views()[0].id;
    until(|| sent(&setup.host, xs).lines.len() == 2).await;
    setup.host.set_footer_settings(ViewFooterSettings {
        wrap: FooterWrap::Never,
        align: FooterAlign::Start,
        ..classic()
    });
    let footer = sent(&setup.host, xs);
    assert_eq!(footer.lines, std::slice::from_ref(&footer.text));
    assert!(!footer.centered);
    assert!(
        footer.tooltip.starts_with(&footer.text),
        "cut: the tooltip says it all"
    );
    assert_eq!(setup.host.footer_height(xs), FOOTER_HEIGHT);
    let without = super::tests::setup(None);
    without.host.on_grid_area(narrow());
    assert_eq!(
        rect(&setup, 0).height + FOOTER_HEIGHT,
        rect(&without, 0).height
    );
}

#[tokio::test]
async fn a_popped_out_window_made_narrower_takes_a_second_line() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let xs = setup.host.views()[0].id;
    setup
        .host
        .pop_out(xs, crate::host::PopOutOptions::default())
        .expect("out");
    assert_eq!(sent(&setup.host, xs).lines.len(), 1);
    // The window's client area is made narrower by hand.
    setup.host.on_view_bounds(
        xs,
        CellRect {
            x: 0,
            y: 0,
            width: 200,
            height: 640,
        },
    );
    until(|| sent(&setup.host, xs).lines.len() == 2).await;
    let window = last_footer(&setup.ui, xs).expect("the window's footer");
    assert_eq!(window.lines.len(), 2);
}

fn last_footer(ui: &RecordingUi, view: ViewId) -> Option<CellFooter> {
    ui.calls().into_iter().rev().find_map(|call| match call {
        UiCall::CellFooter(id, footer) if id == view => footer,
        _ => None,
    })
}

fn grid_count(ui: &RecordingUi) -> usize {
    ui.calls()
        .iter()
        .filter(|call| matches!(call, UiCall::Grid(_)))
        .count()
}

fn built(host: &Host) -> usize {
    host.state().footer.built
}

/// Lets the footer task take the events sent so far.
async fn settle() {
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
}

#[tokio::test]
async fn a_layout_that_keeps_the_widths_writes_no_footer_again() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    settle().await;
    let before = (
        built(&setup.host),
        footer_calls(&setup.ui).len(),
        grid_count(&setup.ui),
    );
    // The area moves down (a second toolbar row): the same widths, nothing to write.
    setup.host.on_grid_area(CellRect { y: 88, ..area() });
    settle().await;
    // A little narrower: the same lines, the texts are not written again.
    setup.host.on_grid_area(CellRect {
        width: 1150,
        ..area()
    });
    settle().await;
    let after = (
        built(&setup.host),
        footer_calls(&setup.ui).len(),
        grid_count(&setup.ui),
    );
    assert_eq!(after, before);
}

#[tokio::test]
async fn a_new_grid_is_fitted_to_its_own_cells_at_once() {
    // One column: every cell is as wide as the narrow area, one line each.
    let mut stacked = config();
    stacked.layout = layout_for(&stacked);
    stacked.layout.columns = 1;
    stacked.layout.rows = 3;
    for (row, cell) in stacked.layout.cells.iter_mut().enumerate() {
        cell.col = 1;
        cell.row = u32::try_from(row).expect("row") + 1;
    }
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(narrow());
    setup.host.open_config(stacked, None).expect("stacked");
    settle().await;
    // The window reports where it laid the views out, as it does after every grid.
    for (index, view) in setup.host.views().iter().enumerate() {
        setup.host.on_view_bounds(view.id, rect(&setup, index));
    }
    settle().await;
    let xs = setup.host.views()[0].id;
    assert_eq!(sent(&setup.host, xs).lines.len(), 1);
    let grids = grid_count(&setup.ui);
    // Three columns: the first grid built already has the second line - no grid after it.
    setup.host.open_config(config(), None).expect("columns");
    let first = setup.ui.last_grid().expect("grid");
    assert_eq!(
        first.cells[0].footer.as_ref().expect("footer").lines.len(),
        2
    );
    settle().await;
    assert_eq!(grid_count(&setup.ui), grids + 1);
}

#[tokio::test]
async fn a_line_more_rebuilds_the_grid_as_a_footer_that_appears() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    settle().await;
    let grids = grid_count(&setup.ui);
    setup.host.on_grid_area(narrow());
    let xs = setup.host.views()[0].id;
    until(|| sent(&setup.host, xs).lines.len() == 2).await;
    settle().await;
    // One path: the grid again, with the strips of two lines; the views get new metrics there.
    assert_eq!(grid_count(&setup.ui), grids + 1);
    let grid = setup.ui.last_grid().expect("grid");
    assert_eq!(
        grid.cells[0].footer.as_ref().expect("footer").lines.len(),
        2
    );
}
