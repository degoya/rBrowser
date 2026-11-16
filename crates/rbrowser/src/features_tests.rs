use std::sync::Arc;

use rb_commands::builtin;
use rb_ipc::IpcErrorCode;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

fn core() -> Arc<Core> {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    Core::new(host, ui, Handle::current(), CoreSetup::default())
}

/// CEF calls `on_context_initialized` on its UI thread, which is outside the Tokio runtime: every
/// feature's start must take the core's runtime handle, never `Handle::current()` (the 0.0.0 test
/// package closed at once with "there is no reactor running" from `devtools::on_start`).
#[test]
fn every_feature_starts_outside_the_runtime_like_on_the_cef_ui_thread() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("a runtime");
    let core = {
        let _inside = runtime.enter();
        core()
    };
    assert!(
        Handle::try_current().is_err(),
        "the test thread is outside the runtime"
    );
    on_start(&core);
}

#[tokio::test]
async fn every_feature_command_is_registered_and_answers_until_its_job_lands() {
    let core = core();
    for def in rb_commands::builtin_features::definitions() {
        let id = def.id.to_string();
        match core.run_command(&id, Value::Null) {
            Ok(_) | Err(CommandError::NotImplemented) | Err(CommandError::Failed(_)) => {}
            Err(other) => panic!("{id}: {other}"),
        }
    }
    assert!(
        builtin::definitions().len() > rb_commands::builtin_features::definitions().len(),
        "the feature commands are part of the builtin definitions"
    );
}

#[tokio::test]
async fn an_unclaimed_message_answers_not_implemented() {
    let core = core();
    let request = json!({ "type": "setSync", "feature": "all", "enabled": false }).to_string();
    let error = crate::ipc::dispatch(&core, &request).expect_err("no job has landed");
    assert_eq!(error.0, IpcErrorCode::NotImplemented);
    // `view.back` belongs to navigation sync (RB-020-06); a shell command to nobody.
    assert!(run_command(&core, "view.focus_address_bar", &Value::Null).is_none());
}

#[tokio::test]
async fn the_hooks_of_start_and_reload_run_without_a_project() {
    let core = core();
    on_start(&core);
    on_config_changed(&core);
}
