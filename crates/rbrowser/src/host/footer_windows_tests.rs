//! The footer and the views outside their cell: a window of its own, its placeholder, the
//! places of a layout mode.

use rb_config::ViewFooterSettings;
use rb_engine::ViewHost as _;

use super::super::ui::recording::UiCall;
use super::tests::{Setup, area, classic, config, footer_calls, setup};
use super::*;

#[tokio::test]
async fn a_window_of_its_own_carries_the_footer_and_follows_the_switch() {
    let setup = setup(Some(classic()));
    let view = setup.host.views()[1].id;
    setup
        .host
        .pop_out(view, crate::host::PopOutOptions::default())
        .expect("out");
    let window = setup
        .ui
        .calls()
        .into_iter()
        .find_map(|call| match call {
            UiCall::PopOut(window) => Some(window),
            _ => None,
        })
        .expect("window");
    let text = "md | 768-1439px | 768 \u{d7} 1024 @2x | min-width 768px | max-width 1439px";
    assert_eq!(window.footer.expect("footer").text, text);
    // The grid does not build a detached view's strip: the switch reaches its window.
    setup.host.set_footer_settings(ViewFooterSettings {
        visible: false,
        ..classic()
    });
    assert_eq!(footer_calls(&setup.ui).last(), Some(&(view, None)));
    setup.host.set_footer_settings(classic());
    assert_eq!(
        footer_calls(&setup.ui).last(),
        Some(&(view, Some(text.to_owned())))
    );
}

#[tokio::test]
async fn a_rebuild_writes_the_footer_of_a_window_of_its_own_again() {
    let setup = setup(Some(classic()));
    let view = setup.host.views()[1].id;
    setup
        .host
        .pop_out(view, crate::host::PopOutOptions::default())
        .expect("out");
    // The config comes back with another colour and another neighbour: a new grid, which
    // builds no strip for the view in its window.
    let mut next = config();
    next.breakpoints[1].color = Some("#000000".to_owned());
    next.breakpoints[2].width = Some(1200);
    setup.host.open_config(next, None).expect("opens");
    let last = setup
        .ui
        .calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            UiCall::CellFooter(id, footer) if id == view => footer,
            _ => None,
        })
        .expect("the window's footer");
    assert_eq!(
        last.text,
        "md | 768-1199px | 768 \u{d7} 1024 @2x | min-width 768px | max-width 1199px"
    );
    assert_eq!(last.background, 0);
}

#[tokio::test]
async fn a_placeholder_cell_has_no_strip() {
    let setup = setup(Some(classic()));
    let view = setup.host.views()[1].id;
    let options = crate::host::PopOutOptions {
        keep_placeholder: true,
        ..crate::host::PopOutOptions::default()
    };
    setup.host.pop_out(view, options).expect("out");
    let grid = setup.ui.last_grid().expect("grid");
    // The cell still names its footer (the window shows it); the grid builds none there.
    assert!(grid.cells[1].footer.is_some());
    assert!(cell_has_strip(&grid, 0));
    assert!(!cell_has_strip(&grid, 1));
    assert!(!cell_has_strip(&grid, 9));
    let none = UiGrid {
        footer_height: 0,
        ..grid
    };
    assert!(!cell_has_strip(&none, 0));
}

#[tokio::test]
async fn a_placed_view_gives_the_footer_the_bottom_of_its_slot() {
    use crate::layout::ModeSpec;
    use crate::layout::spec::ModeKind;
    let strip = || ModeSpec {
        breakpoints: ["xs", "md", "xl"]
            .iter()
            .map(|id| (*id).to_owned())
            .collect(),
        gap: 8,
        kind: ModeKind::Horizontal {
            height: rb_config::StripHeight::Fit,
            zoom: rb_config::Zoom::Fit,
            first: 0,
        },
    };
    let with = setup(Some(classic()));
    let without = setup(None);
    for setup in [&with, &without] {
        setup.host.on_grid_area(area());
        setup.host.set_mode(strip()).expect("mode");
    }
    let rect = |setup: &Setup| setup.host.views()[0].cell.clone().expect("cell").rect;
    assert_eq!(rect(&with).y, rect(&without).y);
    // The footer's height, one line or two in a narrow place (RB-080-53).
    let footer = with.host.footer_height(with.host.views()[0].id);
    assert!(footer >= FOOTER_HEIGHT);
    assert_eq!(rect(&with).height + footer, rect(&without).height);
    // The place itself stays: the strip sits inside it.
    let placed = |setup: &Setup| {
        setup
            .ui
            .last_grid()
            .and_then(|grid| grid.placed)
            .expect("placed")
    };
    assert_eq!(placed(&with), placed(&without));
}
