use rb_ipc::{CoreToShell, NotificationLevel};

use super::*;

const MIB: u64 = 1024 * 1024;

#[test]
fn the_warning_comes_once_when_the_threshold_is_crossed() {
    let mut watch = MemoryWatch::default();
    assert_eq!(watch.observe(2000 * MIB, 3072), None);
    let warning = watch.observe(3100 * MIB, 3072).expect("crossed");
    assert_eq!(
        warning,
        MemoryWarning {
            used_mb: 3100,
            limit_mb: 3072
        }
    );
    assert_eq!(watch.observe(3200 * MIB, 3072), None);
}

#[test]
fn the_warning_rearms_below_nine_tenths_of_the_threshold() {
    let mut watch = MemoryWatch::default();
    watch.observe(3100 * MIB, 3072).expect("crossed");
    // Still close to the threshold: no second warning when it climbs again.
    assert_eq!(watch.observe(3000 * MIB, 3072), None);
    assert_eq!(watch.observe(3100 * MIB, 3072), None);
    assert_eq!(watch.observe(2700 * MIB, 3072), None);
    assert!(watch.observe(3100 * MIB, 3072).is_some());
}

#[test]
fn a_threshold_of_zero_never_warns() {
    let mut watch = MemoryWatch::default();
    assert_eq!(watch.observe(60_000 * MIB, 0), None);
}

#[test]
fn a_lowered_threshold_warns_again_at_once() {
    // The acceptance: a deliberately low threshold shows the warning.
    let mut watch = MemoryWatch::default();
    watch.observe(3100 * MIB, 3072).expect("crossed");
    assert_eq!(
        watch.observe(3100 * MIB, 100),
        Some(MemoryWarning {
            used_mb: 3100,
            limit_mb: 100
        })
    );
}

#[test]
fn the_notification_names_both_numbers_and_links_the_setting() {
    let CoreToShell::Notification(notification) = notification(MemoryWarning {
        used_mb: 3100,
        limit_mb: 3072,
    }) else {
        panic!("a notification");
    };
    assert_eq!(notification.level, NotificationLevel::Warning);
    assert_eq!(notification.message_key, MEMORY_WARNING_KEY);
    assert_eq!(notification.args["usedMb"], "3100");
    assert_eq!(notification.args["limitMb"], "3072");
    assert_eq!(
        notification.settings_key.as_deref(),
        Some("performance.memoryWarningMb")
    );
}

/// RB-080-67: the process count gets a line in the log at the first sample and whenever it has
/// moved by a step since the last line - a leak shows up as it grows, not only at the warning.
#[test]
fn the_process_count_is_logged_when_it_moved_by_a_step() {
    let mut watch = CountWatch::default();
    assert!(watch.observe(25));
    assert!(!watch.observe(36));
    assert!(!watch.observe(25 + COUNT_STEP - 1));
    assert!(watch.observe(25 + COUNT_STEP));
    assert!(!watch.observe(30 + COUNT_STEP));
    assert!(watch.observe(25));
}

/// Review 9: a helper that could not be tied to its job is said once per run of the app, in
/// the shell, not only in the log.
#[test]
fn an_untied_helper_is_said_once() {
    let mut watch = UntiedWatch::default();
    assert_eq!(watch.observe(0), None);
    assert_eq!(watch.observe(1), Some(1));
    assert_eq!(watch.observe(2), None);
    let CoreToShell::Notification(notification) = untied_notification(2) else {
        panic!("a notification");
    };
    assert_eq!(notification.message_key, UNTIED_KEY);
    assert_eq!(notification.level, NotificationLevel::Warning);
    assert_eq!(
        notification.args.get("count").map(String::as_str),
        Some("2")
    );
    assert_eq!(notification.settings_key, None);
}
