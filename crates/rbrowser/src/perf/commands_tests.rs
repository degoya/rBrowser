use std::sync::Arc;

use rb_engine::{CellRect, ViewId};
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn core(user_path: Option<std::path::PathBuf>) -> (Arc<Core>, Arc<RecordingUi>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let setup = CoreSetup {
        user_path,
        ..CoreSetup::default()
    };
    (Core::new(host, ui.clone(), Handle::current(), setup), ui)
}

fn hide_view_one(core: &Core) {
    core.host().on_view_bounds(
        ViewId(1),
        CellRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        },
    );
}

#[tokio::test]
async fn the_toggle_flips_the_setting_and_the_hidden_view_follows() {
    let (core, ui) = core(None);
    crate::perf::on_start(&core);
    hide_view_one(&core);
    assert!(ui.calls().contains(&UiCall::Hidden(ViewId(1), true)));
    let answer = run_command(&core, TOGGLE_THROTTLE, &json!({}))
        .expect("the command is ours")
        .expect("runs");
    assert_eq!(answer, json!({ "throttleHidden": false }));
    assert!(!core.user_settings().performance.throttle_hidden);
    assert!(ui.calls().contains(&UiCall::Hidden(ViewId(1), false)));
    assert!(!core.host().throttle_settings().throttle_hidden);
    // `enabled` sets instead of flipping.
    let answer = run_command(&core, TOGGLE_THROTTLE, &json!({ "enabled": false }))
        .expect("ours")
        .expect("runs");
    assert_eq!(answer, json!({ "throttleHidden": false }));
    let answer = run_command(&core, TOGGLE_THROTTLE, &json!({ "enabled": true }))
        .expect("ours")
        .expect("runs");
    assert_eq!(answer, json!({ "throttleHidden": true }));
    assert!(core.host().throttle_settings().throttle_hidden);
}

#[tokio::test]
async fn the_toggle_keeps_the_choice_in_the_settings_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    std::fs::write(&path, r#"{ "ui": { "theme": "dark" } }"#).expect("written");
    let (core, _ui) = core(Some(path.clone()));
    run_command(&core, TOGGLE_THROTTLE, &json!({}))
        .expect("ours")
        .expect("runs");
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
    assert_eq!(written["performance"]["throttleHidden"], json!(false));
    assert_eq!(written["ui"]["theme"], json!("dark"));
}

#[tokio::test]
async fn a_wrong_argument_is_refused_and_other_commands_are_not_ours() {
    let (core, _ui) = core(None);
    let refused = run_command(&core, TOGGLE_THROTTLE, &json!({ "enabled": "yes" })).expect("ours");
    assert!(refused.is_err());
    assert!(run_command(&core, "view.zoom_in", &json!({})).is_none());
}

#[tokio::test]
async fn the_settings_reach_the_host_at_start() {
    let (core, _ui) = core(None);
    let mut user = core.user_settings();
    user.performance.inactive_fps = 500;
    core.replace_user_settings(user);
    crate::perf::on_start(&core);
    // Clamped: a hand-edited file cannot ask for more than 60.
    assert_eq!(core.host().throttle_settings().inactive_fps, 60);
}
