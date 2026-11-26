use super::*;

fn progress() -> Progress {
    Progress {
        pages: 19,
        breakpoints: 4,
        total: 76,
        done: 38,
        failed: 1,
        pending: 37,
        eta_seconds: Some(12),
    }
}

#[test]
fn the_shell_sees_the_progress_and_the_time_left_only_while_running() {
    let running = shell_status("r", RunState::Running, progress(), "f".to_owned());
    assert_eq!(running.state, SitemapRunState::Running);
    assert_eq!((running.total, running.done, running.failed), (76, 38, 1));
    assert_eq!(running.eta_seconds, Some(12));
    let cancelled = shell_status("r", RunState::Cancelled, progress(), "f".to_owned());
    assert_eq!(cancelled.state, SitemapRunState::Cancelled);
    assert_eq!(cancelled.eta_seconds, None);
}

#[test]
fn a_run_error_keeps_its_code_and_masks_its_message() {
    let failure = tool_failure(&RunError::Discover(
        "plugins.error.http_failed reason=https://user:secret@example.test/".to_owned(),
    ));
    assert_eq!(failure.code, "sitemap.error.discover");
    assert!(!failure.message.contains("secret"), "{}", failure.message);
    assert_eq!(tool_failure(&RunError::Busy).code, "sitemap.error.busy");
}

#[test]
fn the_notices_link_the_sitemap_settings() {
    let CoreToShell::Notification(notification) = notice(
        NotificationLevel::Info,
        FINISHED_KEY,
        &[("done", "76".to_owned())],
    ) else {
        panic!("a notification");
    };
    assert_eq!(notification.message_key, "sitemap.notice.finished");
    assert_eq!(notification.args["done"], "76");
    assert_eq!(notification.settings_key.as_deref(), Some("sitemap"));
}

#[test]
fn the_visible_command_captures_only_the_visible_views() {
    use super::super::tool::CaptureViews;
    let plain = command_args(CAPTURE, &serde_json::json!(null)).expect("plain");
    assert_eq!(plain.views, None);
    let visible = command_args(CAPTURE_VISIBLE, &serde_json::json!(null)).expect("visible");
    assert_eq!(visible.views, Some(CaptureViews::Visible));
    let over = command_args(CAPTURE_VISIBLE, &serde_json::json!({ "views": "all" }))
        .expect("the command's word wins");
    assert_eq!(over.views, Some(CaptureViews::Visible));
    assert!(
        command_args(
            CAPTURE_VISIBLE,
            &serde_json::json!({ "breakpoints": ["mobile"] })
        )
        .is_err()
    );
}
