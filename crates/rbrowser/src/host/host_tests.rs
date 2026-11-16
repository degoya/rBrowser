use std::time::Duration;

use rb_config::ProjectConfig;
use serde_json::json;

use super::nav::marker_url;
use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::layout_plan::NodeKind;
use ui::recording::{RecordingUi, UiCall};

const FIXTURE: &str = include_str!("../../../../tests/fixtures/grid-project/.rbrowser/config.json");

fn fixture() -> ProjectConfig {
    serde_json::from_str(FIXTURE).expect("fixture parses")
}

struct Setup {
    fake: Arc<FakeBrowser>,
    ui: Arc<RecordingUi>,
    host: Arc<Host>,
}

fn setup(config: ProjectConfig) -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config, None).expect("opens");
    for id in 1..=20 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    Setup { fake, ui, host }
}

/// Lets the spawned tasks run until `done` holds (or fails the test after a second).
async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

fn rect(width: u32, height: u32) -> CellRect {
    CellRect {
        x: 8,
        y: 72,
        width,
        height,
    }
}

/// Lays out view 1 (desktop, 1440 wide) and attaches it.
async fn attach_first(setup: &Setup, width: u32) {
    setup.host.on_view_bounds(ViewId(1), rect(width, 300));
    setup.host.on_load_end(ViewId(1), &marker_url(ViewId(1)));
    until(|| {
        !setup
            .fake
            .sent("Emulation.setDeviceMetricsOverride")
            .is_empty()
    })
    .await;
}

#[tokio::test]
async fn the_fixture_opens_six_views_with_their_markers() {
    let setup = setup(fixture());
    let grid = setup.ui.last_grid().expect("a grid was built");
    assert_eq!(grid.cells.len(), 6);
    assert_eq!(grid.cells[0].initial_url, "about:blank#rb-view-1");
    assert_eq!(grid.cells[0].label, "Desktop  1440 \u{d7} 900");
    let mobile = grid
        .cells
        .iter()
        .find(|cell| cell.label.starts_with("Mobile  "))
        .expect("the mobile cell");
    assert_eq!(mobile.label, "Mobile  393 \u{d7} 852 @3x");
    assert!(matches!(
        grid.plan.kind,
        NodeKind::Split {
            horizontal: true,
            ..
        }
    ));
    let views = setup.host.views();
    let ids: Vec<Option<String>> = views.iter().map(|view| view.breakpoint.clone()).collect();
    assert_eq!(ids[0].as_deref(), Some("desktop"));
    assert_eq!(ids[5].as_deref(), Some("mobile-small"));
}

#[tokio::test]
async fn attaching_sends_the_metrics_then_loads_the_start_url() {
    let setup = setup(fixture());
    attach_first(&setup, 480).await;
    let sent = setup.fake.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(sent[0]["sessionId"], "S1");
    assert_eq!(sent[0]["params"]["width"], 1440);
    let scale = sent[0]["params"]["scale"].as_f64().expect("scale");
    assert!((scale - 1.0 / 3.0).abs() < 1e-9);
    assert_eq!(sent[0]["params"]["height"], 900);
    until(|| {
        setup.ui.calls().contains(&UiCall::Navigate(
            ViewId(1),
            "http://127.0.0.1:18765/grid.html".to_owned(),
        ))
    })
    .await;
    let info = setup.host.view(ViewId(1)).expect("view");
    assert!((info.scale - scale).abs() < 1e-9);
}

#[tokio::test]
async fn a_resize_sends_new_metrics_and_an_unchanged_size_sends_none() {
    let setup = setup(fixture());
    attach_first(&setup, 480).await;
    setup.host.on_view_bounds(ViewId(1), rect(360, 300));
    until(|| setup.fake.sent("Emulation.setDeviceMetricsOverride").len() == 2).await;
    let sent = setup.fake.sent("Emulation.setDeviceMetricsOverride");
    assert!((sent[1]["params"]["scale"].as_f64().expect("scale") - 0.25).abs() < 1e-9);
    setup.host.on_view_bounds(ViewId(1), rect(360, 300));
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        setup.fake.sent("Emulation.setDeviceMetricsOverride").len(),
        2
    );
}

#[tokio::test]
async fn after_a_forced_reconnect_the_emulation_is_restored() {
    let setup = setup(fixture());
    attach_first(&setup, 480).await;
    setup.host.reconnect(ViewId(1)).await.expect("detach sent");
    assert_eq!(
        setup.fake.sent("Target.detachFromTarget")[0]["params"]["sessionId"],
        "S1"
    );
    // The browser confirms the detach; the host attaches again and re-applies.
    setup.fake.emit(json!({"method": "Target.detachedFromTarget", "params": {"sessionId": "S1", "targetId": "T1"}}));
    until(|| setup.fake.sent("Emulation.setDeviceMetricsOverride").len() == 2).await;
    let sent = setup.fake.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(sent[1]["sessionId"], "S2");
    assert_eq!(sent[1]["params"], sent[0]["params"]);
    let attaches = setup.fake.sent("Target.attachToTarget");
    assert_eq!(
        attaches[1]["params"]["targetId"], "T1",
        "the captured target, no lookup"
    );
    assert_eq!(setup.fake.sent("Target.getTargets").len(), 1);
}

#[tokio::test]
async fn a_changed_column_count_rebuilds_the_grid_without_new_views() {
    let mut config = fixture();
    let setup = setup(config.clone());
    let mut events = setup.host.subscribe();
    config.layout.columns = 5;
    config.layout.cells[5].col_span = 3;
    setup.host.open_config(config, None).expect("reloads");
    let grid = setup.ui.last_grid().expect("grid");
    let views: Vec<ViewId> = grid.cells.iter().map(|cell| cell.view).collect();
    assert_eq!(views, (1..=6).map(ViewId).collect::<Vec<_>>());
    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        seen.push(event);
    }
    assert_eq!(
        seen,
        vec![ViewEvent::LayoutChanged],
        "no view opened or closed"
    );
}

#[tokio::test]
async fn a_new_breakpoint_in_the_layout_opens_one_view_and_closes_the_dropped_one() {
    let mut config = fixture();
    let setup = setup(config.clone());
    config.layout.cells[5].breakpoint = "desktop".to_owned();
    setup.host.open_config(config, None).expect("reloads");
    let views: Vec<ViewId> = setup.host.views().iter().map(|view| view.id).collect();
    assert_eq!(views[..5], (1..=5).map(ViewId).collect::<Vec<_>>()[..]);
    assert_eq!(views[5], ViewId(7));
    assert!(setup.host.view(ViewId(6)).is_none());
}

#[tokio::test]
async fn an_invalid_layout_changes_nothing() {
    let setup = setup(fixture());
    let mut layout = fixture().layout;
    layout.cells[1].col = 1;
    assert!(matches!(
        setup.host.set_layout(&layout),
        Err(EngineError::Invalid(_))
    ));
    layout = fixture().layout;
    layout.cells[1].breakpoint = "watch".to_owned();
    assert!(setup.host.set_layout(&layout).is_err());
    assert_eq!(setup.host.views().len(), 6);
}

#[tokio::test]
async fn navigation_waits_for_the_session_then_goes_through_the_window() {
    let setup = setup(fixture());
    setup
        .host
        .navigate(Target::View(ViewId(1)), "example.com")
        .expect("navigates");
    assert!(
        !setup
            .ui
            .calls()
            .iter()
            .any(|call| matches!(call, UiCall::Navigate(..)))
    );
    attach_first(&setup, 480).await;
    until(|| {
        setup.ui.calls().contains(&UiCall::Navigate(
            ViewId(1),
            "https://example.com".to_owned(),
        ))
    })
    .await;
    setup
        .host
        .navigate(Target::View(ViewId(1)), "localhost:3000")
        .expect("navigates");
    assert!(setup.ui.calls().contains(&UiCall::Navigate(
        ViewId(1),
        "http://localhost:3000".to_owned()
    )));
    assert!(
        setup
            .host
            .navigate(Target::All, "javascript:alert(1)")
            .is_err()
    );
}

#[tokio::test]
async fn cdp_input_is_scaled_to_the_view() {
    let setup = setup(fixture());
    attach_first(&setup, 360).await;
    setup
        .host
        .cdp_send(
            ViewId(1),
            "Input.dispatchMouseEvent",
            json!({"type": "mousePressed", "x": 1430, "y": 10}),
        )
        .await
        .expect("sent");
    let sent = &setup.fake.sent("Input.dispatchMouseEvent")[0];
    assert_eq!(sent["params"]["x"], json!(357.5));
    assert_eq!(sent["sessionId"], "S1");
    assert!(
        setup
            .host
            .cdp_send(ViewId(2), "Page.reload", json!({}))
            .await
            .is_err(),
        "view 2 is not attached"
    );
}

#[tokio::test]
async fn zoom_and_focus_commands_act_on_the_active_view() {
    let setup = setup(fixture());
    attach_first(&setup, 480).await;
    assert_eq!(setup.host.focus_step(true).expect("focus"), ViewId(1));
    assert_eq!(setup.host.focus_step(false).expect("focus"), ViewId(6));
    assert_eq!(setup.host.focus_step(true).expect("focus"), ViewId(1));
    assert!(setup.ui.calls().contains(&UiCall::Focus(ViewId(6))));
    setup.host.zoom_active(true).expect("zooms");
    until(|| setup.fake.sent("Emulation.setDeviceMetricsOverride").len() == 2).await;
    let sent = setup.fake.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(sent[1]["params"]["scale"], json!(0.5));
    setup.host.zoom_fit_all();
    until(|| setup.fake.sent("Emulation.setDeviceMetricsOverride").len() == 3).await;
    let sent = setup.fake.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(sent[2]["params"], sent[0]["params"]);
}

#[tokio::test]
async fn activating_the_active_view_focuses_it_again_without_an_event() {
    let setup = setup(fixture());
    setup.host.set_active_view(ViewId(6)).expect("active");
    let mut events = setup.host.subscribe();
    let before = setup.ui.calls().len();
    setup.host.set_active_view(ViewId(6)).expect("active again");
    let calls = setup.ui.calls();
    assert_eq!(calls[before..], [UiCall::Focus(ViewId(6))]);
    assert!(
        events.try_recv().is_err(),
        "no ActiveChanged for the same view"
    );
}

#[tokio::test]
async fn an_opened_view_gets_a_column_of_its_own() {
    let setup = setup(fixture());
    let phone = fixture().breakpoints[4].clone();
    let view = setup.host.open_view(&phone).expect("opens");
    assert_eq!(view, ViewId(7));
    let grid = setup.ui.last_grid().expect("grid");
    assert_eq!(grid.cells.len(), 7);
    assert_eq!(grid.cells[6].view, ViewId(7));
}

#[tokio::test]
async fn a_view_without_area_is_hidden_and_shown_again() {
    let setup = setup(fixture());
    setup.host.on_view_bounds(ViewId(2), rect(0, 0));
    setup.host.on_view_bounds(ViewId(2), rect(290, 368));
    let calls = setup.ui.calls();
    assert!(calls.contains(&UiCall::Hidden(ViewId(2), true)));
    assert!(calls.contains(&UiCall::Hidden(ViewId(2), false)));
}

#[tokio::test]
async fn no_metrics_are_sent_before_the_view_is_laid_out() {
    let setup = setup(fixture());
    setup.host.on_load_end(ViewId(1), &marker_url(ViewId(1)));
    until(|| {
        setup
            .host
            .view(ViewId(1))
            .is_some_and(|_| !setup.fake.sent("Target.attachToTarget").is_empty())
    })
    .await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(
        setup
            .fake
            .sent("Emulation.setDeviceMetricsOverride")
            .is_empty()
    );
    setup.host.on_view_bounds(ViewId(1), rect(480, 300));
    until(|| setup.fake.sent("Emulation.setDeviceMetricsOverride").len() == 1).await;
}

#[tokio::test]
async fn a_reload_with_a_changed_breakpoint_or_zoom_reapplies_the_kept_view() {
    let mut config = fixture();
    let setup = setup(config.clone());
    attach_first(&setup, 480).await;
    config.breakpoints[0].width = Some(1920);
    setup
        .host
        .open_config(config.clone(), None)
        .expect("reloads");
    until(|| setup.fake.sent("Emulation.setDeviceMetricsOverride").len() == 2).await;
    let sent = setup.fake.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(sent[1]["params"]["width"], 1920);
    config.layout.cells[0].zoom = rb_config::Zoom::Factor(0.5);
    setup.host.open_config(config, None).expect("reloads");
    until(|| setup.fake.sent("Emulation.setDeviceMetricsOverride").len() == 3).await;
    let sent = setup.fake.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(sent[2]["params"]["scale"], json!(0.5));
}

#[tokio::test]
async fn a_breakpoint_with_only_a_device_gets_the_presets_size_in_the_grid() {
    let mut config = fixture();
    config.breakpoints[0].width = None;
    config.breakpoints[0].height = None;
    config.breakpoints[0].device = Some("iphone-15".to_owned());
    let setup = setup(config);
    let grid = setup.ui.last_grid().expect("a grid was built");
    assert_eq!(grid.cells[0].label, "Desktop  393 \u{d7} 852 @3x");
    attach_first(&setup, 480).await;
    let sent = setup.fake.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(sent[0]["params"]["width"], 393);
    assert_eq!(setup.host.view(ViewId(1)).map(|view| view.width), Some(393));
}
