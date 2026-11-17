use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use rb_config::{FooterField, ProjectConfig, ViewFooterSettings};
use rb_engine::ViewHost as _;
use rb_ipc::{BoundaryKind, DetectedBreakpoint};
use serde_json::json;
use tokio::runtime::Handle;

use super::super::ui::recording::{RecordingUi, UiCall};
use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::view_footer::PALETTE;

pub(super) fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "breakpoints": [
            { "id": "xs", "width": 360, "height": 640 },
            { "id": "md", "width": 768, "height": 1024, "dpr": 2, "color": "#22c55e" },
            { "id": "xl", "width": 1440, "height": 900 }
        ]
    }))
    .expect("config")
}

pub(super) struct Setup {
    pub(super) ui: Arc<RecordingUi>,
    pub(super) host: Arc<Host>,
}

pub(super) fn setup(footer: Option<ViewFooterSettings>) -> Setup {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    if let Some(footer) = footer {
        host.set_footer_settings(footer);
    }
    host.open_config(config(), None).expect("opens");
    Setup { ui, host }
}

/// The footer with the four fields of the default before RB-080-78 - breakpoint, range, size
/// and queries - which these tests read.
pub(super) fn classic() -> ViewFooterSettings {
    ViewFooterSettings {
        fields: BTreeSet::from([
            FooterField::Breakpoint,
            FooterField::Range,
            FooterField::Size,
            FooterField::Queries,
        ]),
        ..ViewFooterSettings::default()
    }
}

fn fields(fields: &[FooterField]) -> ViewFooterSettings {
    ViewFooterSettings {
        visible: true,
        fields: fields.iter().copied().collect::<BTreeSet<_>>(),
        ..classic()
    }
}

/// Lets the footer task run until `done` holds (or fails the test after a second).
async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

pub(super) fn footer_calls(ui: &RecordingUi) -> Vec<(ViewId, Option<String>)> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::CellFooter(view, footer) => Some((view, footer.map(|footer| footer.text))),
            _ => None,
        })
        .collect()
}

pub(super) fn area() -> CellRect {
    CellRect {
        x: 0,
        y: 44,
        width: 1200,
        height: 600,
    }
}

#[tokio::test]
async fn a_host_without_the_setting_draws_no_footer() {
    let setup = setup(None);
    let grid = setup.ui.last_grid().expect("grid");
    assert_eq!(grid.footer_height, 0);
    assert!(grid.cells.iter().all(|cell| cell.footer.is_none()));
    assert_eq!(setup.host.footer_height(ViewId(1)), 0);
}

#[tokio::test]
async fn every_cell_of_the_grid_carries_its_footer() {
    let setup = setup(Some(classic()));
    let grid = setup.ui.last_grid().expect("grid");
    assert_eq!(grid.footer_height, FOOTER_HEIGHT);
    let texts: Vec<&str> = grid
        .cells
        .iter()
        .map(|cell| cell.footer.as_ref().expect("footer").text.as_str())
        .collect();
    assert_eq!(
        texts,
        [
            "xs | 0-767px | 360 \u{d7} 640 | max-width 767px",
            "md | 768-1439px | 768 \u{d7} 1024 @2x | min-width 768px | max-width 1439px",
            "xl | 1440-\u{221e}px | 1440 \u{d7} 900 | min-width 1440px",
        ]
    );
    // The breakpoint's own colour, else the palette's by its place in the list.
    let colors: Vec<u32> = grid
        .cells
        .iter()
        .map(|cell| cell.footer.as_ref().expect("footer").background)
        .collect();
    assert_eq!(colors, [PALETTE[0], 0x0022_c55e, PALETTE[2]]);
}

#[tokio::test]
async fn the_view_gives_the_footer_its_height_in_the_computed_cells() {
    let with = setup(Some(classic()));
    let without = setup(None);
    with.host.on_grid_area(area());
    without.host.on_grid_area(area());
    let (with, without) = (with.host.views(), without.host.views());
    for (with, without) in with.iter().zip(&without) {
        let (with, without) = (
            with.cell.as_ref().expect("cell").rect,
            without.cell.as_ref().expect("cell").rect,
        );
        assert_eq!(with.y, without.y);
        assert_eq!(with.height + FOOTER_HEIGHT, without.height);
    }
}

#[tokio::test]
async fn a_new_size_writes_the_footer_of_that_view_only() {
    let setup = setup(Some(classic()));
    let view = setup.host.views()[1].id;
    setup.host.set_size(view, 800, 600, None).expect("sized");
    until(|| !footer_calls(&setup.ui).is_empty()).await;
    // A free size: no id, the range of the config's neighbours, the cell's colour.
    assert_eq!(
        footer_calls(&setup.ui),
        [(
            view,
            Some("768-1439px | 800 \u{d7} 600 @2x | min-width 768px | max-width 1439px".to_owned())
        )]
    );
    let sent = setup.host.state().footer.sent[&view].clone();
    assert_eq!(sent.background, 0x0022_c55e);
}

fn boundary(width: u32, kind: BoundaryKind, label: Option<&str>) -> DetectedBreakpoint {
    DetectedBreakpoint {
        width,
        kind,
        label: label.map(str::to_owned),
        count: 3,
        source: None,
        heuristic: false,
    }
}

/// What the analyser does when it ran: the boundaries, then the footers.
fn analysed(host: &Host, detected: Vec<DetectedBreakpoint>) {
    host.update_shell_state(move |shell| shell.detected = detected);
    host.footer_analysed();
}

fn grid_count(ui: &RecordingUi) -> usize {
    ui.calls()
        .iter()
        .filter(|call| matches!(call, UiCall::Grid(_)))
        .count()
}

fn grid_footers(ui: &RecordingUi) -> Vec<Option<String>> {
    ui.last_grid()
        .expect("grid")
        .cells
        .into_iter()
        .map(|cell| cell.footer.map(|footer| footer.text))
        .collect()
}

#[tokio::test]
async fn a_view_with_nothing_to_say_has_no_footer_and_keeps_its_height() {
    // The framework's name only, and no analysis yet: the field has no value.
    let setup = setup(Some(fields(&[FooterField::Framework])));
    setup.host.on_grid_area(area());
    let grid = setup.ui.last_grid().expect("grid");
    assert_eq!(grid.footer_height, 0);
    assert!(grid.cells.iter().all(|cell| cell.footer.is_none()));
    let full = setup.host.views()[2].cell.clone().expect("cell").rect;
    // An analysis whose boundaries hold only for the wider views: those get a strip.
    let grids = grid_count(&setup.ui);
    analysed(
        &setup.host,
        vec![
            boundary(768, BoundaryKind::Min, Some("md")),
            boundary(1023, BoundaryKind::Min, Some("lg")),
        ],
    );
    assert_eq!(grid_count(&setup.ui), grids + 1);
    assert_eq!(
        grid_footers(&setup.ui),
        [None, Some("md".to_owned()), Some("lg".to_owned())]
    );
    let views = setup.host.views();
    assert_eq!(setup.host.footer_height(views[0].id), 0);
    assert_eq!(setup.host.footer_height(views[2].id), FOOTER_HEIGHT);
    let rect = |index: usize| views[index].cell.clone().expect("cell").rect;
    assert_eq!(rect(0).height, full.height);
    assert_eq!(rect(2).height + FOOTER_HEIGHT, full.height);
    assert!(footer_calls(&setup.ui).is_empty());
}

#[tokio::test]
async fn an_analysis_adds_the_queries_that_hold() {
    // Since RB-080-78 range and queries are the page's step, both ends from its boundaries.
    let setup = setup(Some(classic()));
    let grids = grid_count(&setup.ui);
    analysed(
        &setup.host,
        vec![
            boundary(768, BoundaryKind::Min, Some("md")),
            boundary(1023, BoundaryKind::Max, None),
        ],
    );
    let views = setup.host.views();
    assert_eq!(
        footer_calls(&setup.ui),
        [
            (
                views[0].id,
                Some("xs | 0-767px | 360 \u{d7} 640 | max-width 767px".to_owned())
            ),
            (
                views[1].id,
                Some(
                    "md | 768-1023px | 768 \u{d7} 1024 @2x | min-width 768px | max-width 1023px"
                        .to_owned()
                )
            ),
            (
                views[2].id,
                Some("xl | 1024-\u{221e}px | 1440 \u{d7} 900 | min-width 1024px".to_owned())
            ),
        ]
    );
    // Every view had its strip already: no new grid.
    assert_eq!(grid_count(&setup.ui), grids);
}

#[tokio::test]
async fn a_view_on_another_document_loses_its_queries_until_the_next_analysis() {
    let setup = setup(Some(fields(&[
        FooterField::Breakpoint,
        FooterField::Queries,
    ])));
    let view = setup.host.views()[1].id;
    setup.host.on_address_change(view, "https://a.test/start");
    let detected = vec![boundary(768, BoundaryKind::Min, None)];
    analysed(&setup.host, detected.clone());
    assert_eq!(
        setup.host.state().footer.sent[&view].text,
        "md | min-width 768px"
    );
    let before = footer_calls(&setup.ui).len();
    // A jump inside the page keeps them.
    setup
        .host
        .on_address_change(view, "https://a.test/start#prices");
    // Another document: they are not known for it - the ends of the range instead.
    setup.host.on_address_change(view, "https://a.test/other");
    until(|| footer_calls(&setup.ui).len() > before).await;
    assert_eq!(
        footer_calls(&setup.ui)[before..],
        [(
            view,
            Some("md | min-width 768px | max-width 1439px".to_owned())
        )]
    );
    // The other views keep theirs; the next analysis brings this one's back.
    let other = setup.host.views()[2].id;
    assert_eq!(
        setup.host.state().footer.sent[&other].text,
        "xl | min-width 768px"
    );
    analysed(&setup.host, detected);
    assert_eq!(
        footer_calls(&setup.ui).last(),
        Some(&(view, Some("md | min-width 768px".to_owned())))
    );
}

#[tokio::test]
async fn a_title_or_a_load_writes_no_footer() {
    let setup = setup(Some(classic()));
    let view = setup.host.views()[0].id;
    setup.host.on_title_change(view, "Home");
    setup.host.on_loading_state(view, false);
    setup.host.set_size(view, 400, 700, None).expect("sized");
    until(|| !footer_calls(&setup.ui).is_empty()).await;
    // Only the size's: the events before it changed nothing a footer says.
    assert_eq!(
        footer_calls(&setup.ui),
        [(
            view,
            Some("0-767px | 400 \u{d7} 700 | max-width 767px".to_owned())
        )]
    );
}

#[tokio::test]
async fn the_widths_follow_a_config_put_in_effect_without_a_new_grid() {
    let setup = setup(Some(fields(&[FooterField::Range])));
    let view = setup.host.views()[0].id;
    assert_eq!(
        setup.host.footer_range(view),
        Some(Range {
            from: 0,
            to: Some(767)
        })
    );
    let mut next = config();
    next.breakpoints[1].width = Some(600);
    setup.host.replace_config(next);
    assert_eq!(
        setup.host.footer_range(view),
        Some(Range {
            from: 0,
            to: Some(599)
        })
    );
    assert!(footer_calls(&setup.ui).contains(&(view, Some("0-599px".to_owned()))));
}

#[tokio::test]
async fn switching_the_footer_off_sends_the_grid_without_it() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let grids = |ui: &RecordingUi| {
        ui.calls()
            .iter()
            .filter(|call| matches!(call, UiCall::Grid(_)))
            .count()
    };
    let before = grids(&setup.ui);
    let with = setup.host.views()[0].cell.clone().expect("cell").rect;
    setup.host.set_footer_settings(ViewFooterSettings {
        visible: false,
        ..classic()
    });
    assert_eq!(grids(&setup.ui), before + 1);
    let grid = setup.ui.last_grid().expect("grid");
    assert_eq!(grid.footer_height, 0);
    assert!(grid.cells.iter().all(|cell| cell.footer.is_none()));
    // The view gets the height back.
    let without = setup.host.views()[0].cell.clone().expect("cell").rect;
    assert_eq!(without.height, with.height + FOOTER_HEIGHT);
    // The same settings again change nothing.
    setup.host.set_footer_settings(ViewFooterSettings {
        visible: false,
        ..classic()
    });
    assert_eq!(grids(&setup.ui), before + 1);
}

#[tokio::test]
async fn other_fields_write_the_strips_again_without_a_new_grid() {
    let setup = setup(Some(classic()));
    let grids = setup
        .ui
        .calls()
        .iter()
        .filter(|call| matches!(call, UiCall::Grid(_)))
        .count();
    setup
        .host
        .set_footer_settings(fields(&[FooterField::Breakpoint]));
    let views = setup.host.views();
    assert_eq!(
        footer_calls(&setup.ui),
        [
            (views[0].id, Some("xs".to_owned())),
            (views[1].id, Some("md".to_owned())),
            (views[2].id, Some("xl".to_owned())),
        ]
    );
    let after = setup
        .ui
        .calls()
        .iter()
        .filter(|call| matches!(call, UiCall::Grid(_)))
        .count();
    assert_eq!(after, grids);
}

#[tokio::test]
async fn views_list_reads_the_range_of_a_view() {
    let setup = setup(None);
    let views = setup.host.views();
    assert_eq!(
        setup.host.footer_range(views[0].id),
        Some(Range {
            from: 0,
            to: Some(767)
        })
    );
    assert_eq!(
        setup.host.footer_range(views[2].id),
        Some(Range {
            from: 1440,
            to: None
        })
    );
    assert_eq!(setup.host.footer_range(ViewId(99)), None);
}
