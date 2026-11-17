use std::time::Duration;

use rb_config::ProjectConfig;
use rb_engine::{CellRect, ViewHost};
use tokio::runtime::Handle;

use super::super::nav::marker_url;
use super::super::ui::recording::{RecordingUi, UiCall};
use super::*;
use crate::cdp::fake::FakeBrowser;

const FIXTURE: &str = include_str!("../../../../tests/fixtures/grid-project/.rbrowser/config.json");

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
    for id in 1..=6 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    Setup { fake, ui, host }
}

fn rect(width: u32, height: u32) -> CellRect {
    CellRect {
        x: 8,
        y: 72,
        width,
        height,
    }
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

fn hidden_calls(setup: &Setup, view: u32) -> Vec<bool> {
    setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Hidden(id, hidden) if id == ViewId(view) => Some(hidden),
            _ => None,
        })
        .collect()
}

async fn attach(setup: &Setup, view: u32) {
    setup.host.on_view_bounds(ViewId(view), rect(300, 300));
    setup
        .host
        .on_load_end(ViewId(view), &marker_url(ViewId(view)));
    until(|| {
        setup
            .host
            .state()
            .views
            .get(&ViewId(view))
            .is_some_and(|entry| entry.session.is_some() && !entry.attaching)
    })
    .await;
}

#[tokio::test]
async fn throttling_off_shows_a_hidden_view_and_on_hides_it_again() {
    let setup = setup();
    setup.host.on_view_bounds(ViewId(2), rect(0, 0));
    assert_eq!(hidden_calls(&setup, 2), vec![true]);
    assert_eq!(setup.host.throttle_state(ViewId(2)), ThrottleState::Hidden);
    let on = setup.host.throttle_settings();
    setup.host.set_throttle_settings(ThrottleSettings {
        throttle_hidden: false,
        ..on
    });
    assert_eq!(hidden_calls(&setup, 2), vec![true, false]);
    assert_eq!(setup.host.throttle_state(ViewId(2)), ThrottleState::Visible);
    // Laid out again without area while off: it keeps rendering.
    setup.host.on_view_bounds(ViewId(2), rect(0, 1));
    assert_eq!(hidden_calls(&setup, 2), vec![true, false]);
    setup.host.set_throttle_settings(on);
    assert_eq!(hidden_calls(&setup, 2), vec![true, false, true]);
    // The capture reads the reported state (`capture_target`).
    assert!(setup.host.state().views[&ViewId(2)].hidden);
}

#[tokio::test]
async fn the_same_settings_again_change_nothing() {
    let setup = setup();
    setup.host.on_view_bounds(ViewId(2), rect(0, 0));
    setup
        .host
        .set_throttle_settings(setup.host.throttle_settings());
    assert_eq!(hidden_calls(&setup, 2), vec![true]);
}

#[tokio::test]
async fn inactive_views_get_the_cpu_budget_and_the_new_leader_loses_it() {
    let setup = setup();
    attach(&setup, 1).await;
    attach(&setup, 2).await;
    setup.host.set_active_view(ViewId(1)).expect("known view");
    setup.host.set_throttle_settings(ThrottleSettings {
        throttle_hidden: true,
        inactive_fps: 30,
    });
    let session_of = |view: u32| {
        setup.host.state().views[&ViewId(view)]
            .session
            .clone()
            .expect("attached")
    };
    let rates = |setup: &Setup| {
        setup
            .fake
            .sent("Emulation.setCPUThrottlingRate")
            .into_iter()
            .map(|message| {
                (
                    message["sessionId"].as_str().unwrap_or_default().to_owned(),
                    message["params"]["rate"].as_f64().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>()
    };
    until(|| rates(&setup).contains(&(session_of(2), 2.0))).await;
    assert!(
        !rates(&setup)
            .iter()
            .any(|(session, _)| *session == session_of(1))
    );
    assert_eq!(
        setup.host.throttle_state(ViewId(2)),
        ThrottleState::CpuLimited { fps: 30 }
    );
    setup.host.set_active_view(ViewId(2)).expect("known view");
    until(|| rates(&setup).contains(&(session_of(2), 1.0))).await;
    until(|| rates(&setup).contains(&(session_of(1), 2.0))).await;
    assert_eq!(setup.host.throttle_state(ViewId(2)), ThrottleState::Visible);
}
