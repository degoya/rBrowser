use serde_json::json;

use super::*;
use crate::{CoreToShell, ShellToCore, parse_request};

fn notice(level: NotificationLevel) -> Notification {
    Notification {
        level,
        message_key: "shell.error.request_failed".into(),
        args: [("code".to_owned(), "404".to_owned())].into(),
        settings_key: Some("ui.theme".into()),
    }
}

#[test]
fn a_status_message_is_the_notification_numbered_and_stamped() {
    let message = StatusMessage::new(3, 1_700_000_000_000, notice(NotificationLevel::Error));
    assert_eq!(message.id, 3);
    assert_eq!(message.at, 1_700_000_000_000);
    assert_eq!(message.notification(), notice(NotificationLevel::Error));
}

#[test]
fn the_event_and_the_answer_are_tagged_camel_case() {
    let message = StatusMessage::new(3, 1_700_000_000_000, notice(NotificationLevel::Success));
    assert_eq!(
        serde_json::to_value(CoreToShell::StatusMessage(message.clone())).expect("serializes"),
        json!({
            "type": "statusMessage",
            "id": 3,
            "at": 1_700_000_000_000_u64,
            "level": "success",
            "messageKey": "shell.error.request_failed",
            "args": { "code": "404" },
            "settingsKey": "ui.theme"
        })
    );
    let answer = CoreToShell::StatusMessages {
        items: vec![message],
    };
    let value = serde_json::to_value(&answer).expect("serializes");
    assert_eq!(value["type"], "statusMessages");
    assert_eq!(value["items"][0]["id"], 3);
}

#[test]
fn the_shell_asks_for_the_log_and_reports_its_own_messages() {
    assert_eq!(
        parse_request(r#"{"type":"getStatusMessages"}"#).expect("parses"),
        ShellToCore::GetStatusMessages
    );
    assert_eq!(
        parse_request(r#"{"type":"dismissHistory"}"#).expect("parses"),
        ShellToCore::DismissHistory
    );
    let report = parse_request(
        r#"{"type":"reportStatus","level":"warning","messageKey":"shell.error.no_answer"}"#,
    )
    .expect("parses");
    assert_eq!(
        report,
        ShellToCore::ReportStatus(Notification {
            level: NotificationLevel::Warning,
            message_key: "shell.error.no_answer".into(),
            args: std::collections::BTreeMap::new(),
            settings_key: None,
        })
    );
}

/// RB-090-71: a message may carry a button that runs a command; without one the field is not
/// written, so every message before stays as it was.
#[test]
fn a_status_message_may_carry_a_button_that_runs_a_command() {
    let plain = StatusMessage::new(4, 1_700_000_000_000, notice(NotificationLevel::Warning));
    assert_eq!(plain.action, None);
    assert!(
        serde_json::to_value(&plain)
            .expect("serializes")
            .get("action")
            .is_none()
    );
    let action = StatusAction {
        command: "portal.add_asset_origin".into(),
        label_key: "portal.offer.add".into(),
        label_args: [("origin".to_owned(), "http://localhost:5173".to_owned())].into(),
        args: json!({ "environment": "local", "origin": "http://localhost:5173" }),
    };
    let message = plain.with_action(Some(action.clone()));
    let value =
        serde_json::to_value(CoreToShell::StatusMessage(message.clone())).expect("serializes");
    assert_eq!(
        value["action"],
        json!({
            "command": "portal.add_asset_origin",
            "labelKey": "portal.offer.add",
            "labelArgs": { "origin": "http://localhost:5173" },
            "args": { "environment": "local", "origin": "http://localhost:5173" }
        })
    );
    assert_eq!(
        serde_json::from_value::<StatusMessage>(value).expect("parses"),
        message
    );
    assert_eq!(message.notification(), notice(NotificationLevel::Warning));
    let bare = StatusAction {
        label_args: std::collections::BTreeMap::new(),
        args: serde_json::Value::Null,
        ..action
    };
    assert_eq!(
        serde_json::to_value(&bare).expect("serializes"),
        json!({ "command": "portal.add_asset_origin", "labelKey": "portal.offer.add" })
    );
}

/// RB-093-04: a message may carry a copy button beside its command's button; it names the
/// report whose Markdown the status bar asks for, never the text itself, and without one the
/// field is not written.
#[test]
fn a_status_message_may_carry_a_copy_button_that_names_a_report() {
    let plain = StatusMessage::new(5, 1_700_000_000_000, notice(NotificationLevel::Success));
    assert_eq!(plain.copy, None);
    assert!(
        serde_json::to_value(&plain)
            .expect("serializes")
            .get("copy")
            .is_none()
    );
    let message = plain.with_copy(Some(StatusCopy {
        label_key: "report.action.copy_markdown".into(),
        report: "2026-11-20_14-03-07".into(),
    }));
    let value =
        serde_json::to_value(CoreToShell::StatusMessage(message.clone())).expect("serializes");
    assert_eq!(
        value["copy"],
        json!({ "labelKey": "report.action.copy_markdown", "report": "2026-11-20_14-03-07" })
    );
    assert_eq!(
        serde_json::from_value::<StatusMessage>(value).expect("parses"),
        message
    );
    assert_eq!(message.notification(), notice(NotificationLevel::Success));
}
