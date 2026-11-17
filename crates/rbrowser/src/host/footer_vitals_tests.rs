//! The vitals bar below the breakpoint bar (RB-090-71): a line of the same strip, its height
//! taken from the view like the breakpoint bar's, switched on its own (`ui.viewFooter.vitals`).

use std::collections::BTreeMap;

use rb_config::ViewFooterSettings;
use rb_engine::{CellRect, ViewHost as _, ViewId};
use rb_sync::VitalMetric;

use super::super::Host;
use super::tests::{Setup, area, classic, setup};
use crate::cell_info::{Piece, PieceAction};
use crate::view_footer::{CellFooter, FOOTER_HEIGHT};

fn sent(host: &Host, view: ViewId) -> Option<CellFooter> {
    host.state().footer.sent.get(&view).cloned()
}

fn rect(setup: &Setup, index: usize) -> CellRect {
    setup.host.views()[index].cell.clone().expect("cell").rect
}

fn piece(action: PieceAction) -> Piece {
    Piece {
        text: "\u{25cf} LCP 1.20 s".to_owned(),
        tooltip: "LCP 1.20 s: good".to_owned(),
        action,
    }
}

/// Every view of `setup` with a vitals bar.
fn with_vitals(setup: &Setup, action: PieceAction) -> ViewId {
    let views: Vec<ViewId> = setup.host.views().iter().map(|view| view.id).collect();
    let bars: BTreeMap<ViewId, Piece> = views.iter().map(|view| (*view, piece(action))).collect();
    setup.host.set_footer_vitals(bars);
    views[0]
}

#[tokio::test]
async fn the_vitals_bar_stacks_below_the_breakpoint_bar_and_the_view_gives_its_line() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let before = rect(&setup, 0);
    let view = with_vitals(&setup, PieceAction::Activate);
    let footer = sent(&setup.host, view).expect("a footer");
    assert_eq!(footer.lines.len(), 1, "the breakpoint bar keeps its line");
    assert!(footer.vitals_shown);
    assert_eq!(
        footer.vitals.as_ref().map(|piece| piece.text.as_str()),
        Some("\u{25cf} LCP 1.20 s")
    );
    assert_eq!(setup.host.footer_height(view), 2 * FOOTER_HEIGHT);
    // The view gives the line; its cell keeps its top, so no view overlaps another.
    let after = rect(&setup, 0);
    assert_eq!(after.y, before.y);
    assert_eq!(after.height + FOOTER_HEIGHT, before.height);
}

#[tokio::test]
async fn the_vitals_bar_shows_alone_while_the_breakpoint_bar_is_off() {
    let setup = setup(Some(ViewFooterSettings {
        visible: false,
        ..classic()
    }));
    setup.host.on_grid_area(area());
    let view = with_vitals(&setup, PieceAction::Activate);
    let footer = sent(&setup.host, view).expect("the vitals bar alone");
    assert!(footer.text.is_empty(), "no breakpoint bar: {}", footer.text);
    assert!(footer.bar_lines().is_empty());
    assert!(footer.vitals_shown);
    assert_eq!(setup.host.footer_height(view), FOOTER_HEIGHT);
    let grid = setup.ui.last_grid().expect("grid");
    assert_eq!(grid.footer_height, FOOTER_HEIGHT);
    assert!(grid.cells.iter().all(|cell| cell.footer.is_some()));
}

#[tokio::test]
async fn off_the_vitals_bar_gives_its_line_back() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let view = with_vitals(&setup, PieceAction::Activate);
    assert_eq!(setup.host.footer_height(view), 2 * FOOTER_HEIGHT);
    setup.host.set_footer_settings(ViewFooterSettings {
        vitals: false,
        ..classic()
    });
    let footer = sent(&setup.host, view).expect("the breakpoint bar");
    assert!(footer.vitals.is_none());
    assert_eq!(setup.host.footer_height(view), FOOTER_HEIGHT);
    // Both off: no footer, the view has its whole height.
    setup.host.set_footer_settings(ViewFooterSettings {
        visible: false,
        vitals: false,
        ..classic()
    });
    assert_eq!(sent(&setup.host, view), None);
    assert_eq!(setup.host.footer_height(view), 0);
}

#[tokio::test]
async fn the_same_vitals_again_write_nothing() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    with_vitals(&setup, PieceAction::Activate);
    let calls = setup.ui.calls().len();
    with_vitals(&setup, PieceAction::Activate);
    assert_eq!(setup.ui.calls().len(), calls);
}

#[tokio::test]
async fn a_click_on_the_bar_does_what_its_words_say() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let view = with_vitals(&setup, PieceAction::Mark(VitalMetric::Cls));
    assert_eq!(
        setup.host.footer_vitals_action(view),
        PieceAction::Mark(VitalMetric::Cls)
    );
    assert_eq!(
        setup.host.footer_vitals_action(ViewId(999)),
        PieceAction::Activate,
        "no bar: the click activates"
    );
}

#[tokio::test]
async fn a_new_reading_of_one_view_rewrites_only_that_views_footer() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let first = with_vitals(&setup, PieceAction::Activate);
    let built = setup.host.state().footer.built;
    let mut bars: BTreeMap<ViewId, Piece> = setup
        .host
        .views()
        .iter()
        .map(|view| (view.id, piece(PieceAction::Activate)))
        .collect();
    if let Some(bar) = bars.get_mut(&first) {
        bar.text = "\u{25b2} LCP 4.20 s".to_owned();
    }
    setup.host.set_footer_vitals(bars);
    assert_eq!(
        setup.host.state().footer.built,
        built + 1,
        "one view's bar changed: one footer written"
    );
    assert_eq!(
        sent(&setup.host, first).and_then(|footer| footer.vitals.map(|piece| piece.text)),
        Some("\u{25b2} LCP 4.20 s".to_owned())
    );
}
