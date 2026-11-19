use super::*;

fn started() -> CaptureStarted {
    CaptureStarted {
        run_id: "r1".to_owned(),
        folder: "f".to_owned(),
        pending: 1400,
        pages: 140,
        breakpoints: 10,
    }
}

#[test]
fn a_start_says_how_many_pages_in_how_many_breakpoints() {
    let CoreToShell::Notification(notification) = started_notice(&started()) else {
        panic!("a notification");
    };
    assert_eq!(notification.level, NotificationLevel::Info);
    assert_eq!(notification.message_key, "sitemap.notice.started");
    assert_eq!(notification.args["pages"], "140");
    assert_eq!(notification.args["breakpoints"], "10");
    assert_eq!(notification.args["pending"], "1400");
    assert_eq!(notification.settings_key.as_deref(), Some("sitemap"));
}

#[test]
fn a_second_start_shows_the_running_capture_instead_of_only_failing() {
    let [open, said] = busy_messages();
    assert_eq!(
        open,
        CoreToShell::UiCommand {
            id: "panels.sitemap".to_owned(),
            args: None,
        }
    );
    let CoreToShell::Notification(notification) = said else {
        panic!("a notification");
    };
    assert_eq!(notification.level, NotificationLevel::Info);
    assert_eq!(notification.message_key, "sitemap.notice.busy");
}
