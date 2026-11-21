use std::sync::Arc;

use rb_commands::CommandError;
use rb_engine::ViewId;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

/// A window with the default config's three views, none attached: the commands keep what is
/// asked for the attach.
fn core() -> Arc<Core> {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    // The window's hooks are the process's: start from the real time and 100 %.
    for view in 1..=3 {
        super::super::hooks().forget(ViewId(view));
    }
    Core::new(host, ui, Handle::current(), CoreSetup::default())
}

/// Runs `command` as the registry does, on a thread that may wait on the runtime.
async fn run(
    core: &Arc<Core>,
    command: fn(&Core, &Value) -> Result<Value, CommandError>,
    args: Value,
) -> Result<Value, CommandError> {
    let core = core.clone();
    tokio::task::spawn_blocking(move || command(&core, &args))
        .await
        .expect("the command thread")
}

fn shown(core: &Core) -> Option<rb_ipc::TimeControlActivity> {
    core.host().shell_state().time_control
}

#[tokio::test(flavor = "multi_thread")]
async fn the_clock_shortcut_freezes_the_moment_and_takes_it_back() {
    let core = core();
    let answer = run(&core, clock, Value::Null).await.expect("set");
    assert_eq!(answer["clock"]["local"], false);
    assert_eq!(answer["clock"]["speed"], 0.0);
    assert_eq!(answer["unsupported"], json!([]));
    assert!(shown(&core).and_then(|shown| shown.clock).is_some());
    let answer = run(&core, clock, Value::Null).await.expect("reset");
    assert_eq!(answer["clock"], Value::Null);
    assert_eq!(shown(&core), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_clock_without_a_zone_is_each_pages_local_time_until_the_reset() {
    let core = core();
    let answer = run(&core, clock, json!({ "time": "2026-12-24T10:00" }))
        .await
        .expect("set");
    assert_eq!(
        answer["clock"],
        json!({ "start": "2026-12-24T10:00:00", "local": true, "speed": 0.0 })
    );
    let activity = shown(&core).and_then(|shown| shown.clock).expect("shown");
    assert_eq!(activity.start, "2026-12-24T10:00:00");
    run(&core, clock, json!({ "reset": true }))
        .await
        .expect("reset");
    assert_eq!(shown(&core), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_animation_shortcut_steps_through_the_rates_in_every_view() {
    let core = core();
    let answer = run(&core, animations, Value::Null).await.expect("stop");
    assert_eq!(answer["rate"], 0.0);
    assert_eq!(answer["views"], json!([1, 2, 3]));
    let rates: Vec<(u32, f64)> = shown(&core)
        .expect("shown")
        .animations
        .iter()
        .map(|entry| (entry.view.0, entry.rate))
        .collect();
    assert_eq!(rates, [(1, 0.0), (2, 0.0), (3, 0.0)]);
    let answer = run(&core, animations, Value::Null).await.expect("slower");
    assert_eq!(answer["rate"], 0.1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rate_for_one_view_and_100_percent_for_all_again() {
    let core = core();
    run(&core, animations, json!({ "target": 2, "rate": 0.25 }))
        .await
        .expect("one view");
    let rates: Vec<(u32, f64)> = shown(&core)
        .expect("shown")
        .animations
        .iter()
        .map(|entry| (entry.view.0, entry.rate))
        .collect();
    assert_eq!(rates, [(2, 0.25)]);
    run(&core, animations, json!({ "target": "all", "rate": 1 }))
        .await
        .expect("all");
    assert_eq!(shown(&core), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_step_names_the_views_without_a_page_yet() {
    let core = core();
    let answer = run(&core, animations, json!({ "step": 40 }))
        .await
        .expect("step");
    assert_eq!(answer["step"], 40);
    assert_eq!(answer["moved"], 0);
    assert_eq!(answer["views"], json!([]));
    assert_eq!(answer["failed"].as_array().map(Vec::len), Some(3));
}

#[tokio::test(flavor = "multi_thread")]
async fn what_the_commands_cannot_read_is_refused() {
    let core = core();
    for (command, args) in [
        (clock as fn(&Core, &Value) -> _, json!({ "time": "soon" })),
        (clock, json!({ "reset": true, "speed": 2 })),
        (animations, json!({ "rate": 20 })),
        (animations, json!({ "target": 99, "rate": 0 })),
    ] {
        let refused = run(&core, command, args.clone()).await;
        assert!(
            matches!(refused, Err(CommandError::InvalidArgs(_))),
            "{args}: {refused:?}"
        );
    }
    assert_eq!(shown(&core), None);
}
