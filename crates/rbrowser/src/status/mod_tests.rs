use std::sync::Arc;

use rb_ipc::{IpcErrorCode, Notification, NotificationLevel};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};
use crate::page_window::PageWindow;

fn core() -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    (ui, core)
}

/// The messages sent to the shell pages and to the status bar, as JSON.
fn sent(ui: &RecordingUi) -> Vec<Value> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) | UiCall::StatusEvent(script) => {
                let start = script.find("detail:")? + "detail:".len();
                let end = script.rfind("}));")?;
                serde_json::from_str(&script[start..end]).ok()
            }
            _ => None,
        })
        .collect()
}

fn notice(key: &str, level: NotificationLevel) -> Notification {
    Notification {
        level,
        message_key: key.to_owned(),
        args: std::collections::BTreeMap::new(),
        settings_key: None,
    }
}

#[tokio::test]
async fn every_notification_reaches_the_log_and_the_bar_at_once_and_only_the_bar() {
    let (ui, core) = core();
    // Before the shell is ready: the toolbar's `notification` waits, the bar's event does not.
    core.notify(CoreToShell::Notification(notice(
        "shell.error.no_answer",
        NotificationLevel::Error,
    )));
    let items = core.status_log().items();
    let last = items.last().expect("logged");
    assert_eq!(last.message_key, "shell.error.no_answer");
    let events = sent(&ui);
    let event = events
        .iter()
        .find(|event| event["type"] == "statusMessage" && event["id"] == json!(last.id))
        .expect("the bar's event");
    assert_eq!(event["level"], "error");
    // The toolbar and the cell labels never get it.
    assert!(
        !ui.calls()
            .iter()
            .any(|call| matches!(call, UiCall::Shell(script) if script.contains("statusMessage")))
    );
}

#[tokio::test]
async fn the_bar_reads_the_log_and_the_shell_reports_into_it() {
    let (_ui, core) = core();
    let report =
        r#"{"type":"reportStatus","level":"warning","messageKey":"shell.error.no_answer"}"#;
    let answer = crate::ipc::dispatch(&core, report).expect("ack");
    assert!(answer.contains("\"ack\""), "{answer}");
    let answer = crate::ipc::dispatch(&core, r#"{"type":"getStatusMessages"}"#).expect("answers");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(answer["type"], "statusMessages");
    let items = answer["items"].as_array().expect("a list");
    assert_eq!(
        items.last().map(|item| item["messageKey"].clone()),
        Some(json!("shell.error.no_answer"))
    );
}

#[tokio::test]
async fn a_malformed_report_is_refused() {
    let (_ui, core) = core();
    let before = core.status_log().items().len();
    let report = r#"{"type":"reportStatus","level":"info","messageKey":"<b>x</b>"}"#;
    let (code, _) = crate::ipc::dispatch(&core, report).expect_err("refused");
    assert_eq!(code, IpcErrorCode::BadRequest);
    assert_eq!(core.status_log().items().len(), before);
}

#[tokio::test]
async fn the_window_and_the_log_follow_the_settings() {
    let (ui, core) = core();
    let mut user = core.user_settings();
    user.ui.status_bar.visible = false;
    user.ui.status_bar.history_size = 10;
    core.replace_user_settings(user);
    assert!(ui.calls().contains(&UiCall::StatusBar(false)));
    for n in 0..15 {
        core.notify(CoreToShell::Notification(notice(
            &format!("a.n{n}"),
            NotificationLevel::Info,
        )));
    }
    assert_eq!(core.status_log().items().len(), 10);
}

#[tokio::test]
async fn the_toggle_switches_the_setting_and_the_window() {
    let (ui, core) = core();
    let answer = core
        .run_command(defs::TOGGLE_BAR, Value::Null)
        .expect("runs");
    assert_eq!(answer, json!({ "visible": false }));
    assert!(!core.user_settings().ui.status_bar.visible);
    assert!(ui.calls().contains(&UiCall::StatusBar(false)));
    core.run_command(defs::TOGGLE_BAR, Value::Null)
        .expect("runs");
    assert!(core.user_settings().ui.status_bar.visible);
    assert!(ui.calls().contains(&UiCall::StatusBar(true)));
}

#[tokio::test]
async fn the_history_opens_above_the_bar_and_the_bar_counts_it_read() {
    let (ui, core) = core();
    core.run_command(defs::HISTORY, Value::Null).expect("runs");
    let history = PageWindow::History;
    assert!(
        ui.calls()
            .contains(&UiCall::PageWindow(history, history.url()))
    );
    assert!(
        sent(&ui)
            .iter()
            .any(|event| event["type"] == "uiCommand" && event["id"] == defs::HISTORY)
    );
}

#[tokio::test]
async fn escape_in_the_history_hides_it() {
    let (ui, core) = core();
    crate::ipc::dispatch(&core, r#"{"type":"dismissHistory"}"#).expect("ack");
    assert!(
        ui.calls()
            .contains(&UiCall::HidePageWindow(PageWindow::History))
    );
}
