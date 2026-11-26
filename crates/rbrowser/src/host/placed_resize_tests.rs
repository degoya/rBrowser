//! The views of a layout mode follow a size written by any path, with one metrics call per
//! change (RB-080-54), and a view a gesture brings into the grid area is unpaused at once
//! (RB-080-55).

use std::sync::Arc;
use std::time::Duration;

use rb_config::{ProjectConfig, default_item_zoom};
use rb_engine::ViewHost as _;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::host::LABEL_HEIGHT;
use crate::host::nav::marker_url;
use crate::host::ui::recording::{RecordingUi, UiCall};
use crate::layout::canvas::Camera;
use crate::layout::spec::ModeKind;

const FIXTURE: &str = include_str!("../../../../tests/fixtures/grid-project/.rbrowser/config.json");

const AREA: CellRect = CellRect {
    x: 0,
    y: 44,
    width: 1600,
    height: 900,
};

struct Setup {
    fake: Arc<FakeBrowser>,
    ui: Arc<RecordingUi>,
    host: Arc<Host>,
}

fn setup() -> Setup {
    let config: ProjectConfig = serde_json::from_str(FIXTURE).expect("fixture parses");
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config, None).expect("opens");
    for id in 1..=20 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    host.on_grid_area(AREA);
    Setup { fake, ui, host }
}

fn canvas(breakpoints: &[&str], camera: Camera) -> ModeSpec {
    ModeSpec {
        breakpoints: breakpoints.iter().map(|id| (*id).to_owned()).collect(),
        gap: 8,
        kind: ModeKind::Canvas {
            snap: 8,
            items: Vec::new(),
            camera: Some(camera),
            partial: true,
        },
    }
}

fn places(ui: &RecordingUi) -> usize {
    ui.calls()
        .iter()
        .filter(|call| matches!(call, UiCall::Place(_)))
        .count()
}

fn last_place(ui: &RecordingUi, view: ViewId) -> PlacedView {
    ui.calls()
        .into_iter()
        .rev()
        .find_map(|call| match call {
            UiCall::Place(placed) => placed.into_iter().find(|place| place.view == view),
            _ => None,
        })
        .expect("the view was placed")
}

fn metrics(setup: &Setup) -> usize {
    setup.fake.sent("Emulation.setDeviceMetricsOverride").len()
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

fn off_screen(host: &Host, view: ViewId) -> bool {
    host.state().views[&view].off_screen
}

/// The grid the window builds carries the area its places were computed for (review, 7).
#[tokio::test]
async fn the_places_come_with_their_area() {
    let setup = setup();
    setup
        .host
        .set_mode(canvas(&["mobile"], Camera::default()))
        .expect("canvas");
    let grid = setup.ui.last_grid().expect("a grid");
    assert!(grid.placed.is_some());
    assert_eq!(grid.area, Some(AREA));
}

/// A size written by a path that is none of the host's emulation methods still places the
/// views, once (review, 6).
#[tokio::test]
async fn a_size_written_by_any_path_places_the_views_once() {
    let setup = setup();
    let order = setup
        .host
        .set_mode(canvas(&["mobile", "tablet"], Camera::default()))
        .expect("canvas");
    let mobile = order[0];
    let before = places(&setup.ui);
    {
        let mut state = setup.host.state();
        let entry = state.views.get_mut(&mobile).expect("mobile");
        entry.emulation.width = 500;
        entry.emulation.height = 1000;
    }
    setup.host.spawn_apply(mobile, false);
    assert_eq!(places(&setup.ui), before + 1);
    let zoom = default_item_zoom();
    let outer = last_place(&setup.ui, mobile).outer;
    assert_eq!(outer.width, (500.0 * zoom).round() as u32);
    assert_eq!(outer.height, (1000.0 * zoom).round() as u32 + LABEL_HEIGHT);
    setup.host.spawn_apply(mobile, false);
    assert_eq!(
        places(&setup.ui),
        before + 1,
        "the same size places nothing"
    );
}

/// One size change, one `Emulation.setDeviceMetricsOverride` (review, 2).
#[tokio::test]
async fn one_size_change_sends_the_metrics_once() {
    let setup = setup();
    let order = setup
        .host
        .set_mode(canvas(&["mobile"], Camera::default()))
        .expect("canvas");
    let mobile = order[0];
    setup.host.on_load_end(mobile, &marker_url(mobile));
    until(|| metrics(&setup) > 0).await;
    // The attach and the applies the layout asked for ended (RB-099-18).
    until(|| setup.host.applies_settled(mobile)).await;
    let before = metrics(&setup);
    setup
        .host
        .set_size(mobile, 500, 1000, None)
        .expect("a size");
    until(|| metrics(&setup) > before).await;
    until(|| setup.host.applies_settled(mobile)).await;
    assert_eq!(metrics(&setup), before + 1);
}

/// A gesture brings a paused view into the area: it runs at once; it is paused again only
/// when the gesture ends (review, 3).
#[tokio::test]
async fn a_view_a_gesture_brings_into_the_area_runs_at_once() {
    let setup = setup();
    let far = Camera {
        x: 100_000.0,
        y: 0.0,
        zoom: 1.0,
    };
    let order = setup
        .host
        .set_mode(canvas(&["mobile"], far))
        .expect("canvas");
    let mobile = order[0];
    assert!(off_screen(&setup.host, mobile), "outside: paused");
    let inside = PlacedView {
        view: mobile,
        outer: CellRect {
            x: 10,
            y: 54,
            width: 200,
            height: 420,
        },
        visible: true,
    };
    setup.host.place_live(AREA, vec![inside]);
    assert!(!off_screen(&setup.host, mobile), "in the area: runs");
    assert_eq!(last_place(&setup.ui, mobile), inside);
    setup.host.place_live(
        AREA,
        vec![PlacedView {
            visible: false,
            ..inside
        }],
    );
    assert!(
        !off_screen(&setup.host, mobile),
        "paused again only at the gesture's end"
    );
    setup.host.update_mode(|_| {});
    assert!(off_screen(&setup.host, mobile), "the end places it outside");
}
