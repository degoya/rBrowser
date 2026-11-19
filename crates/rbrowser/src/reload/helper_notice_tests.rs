use super::*;
use crate::reload::classify::HotMode;

fn info(status: WatcherStatus) -> WatcherInfo {
    WatcherInfo::new(status, false, HotMode::Auto, None)
}

fn parts(message: CoreToShell) -> rb_ipc::Notification {
    match message {
        CoreToShell::Notification(notification) => notification,
        other => panic!("{other:?}"),
    }
}

/// RB-090-85: `wsl.restart_helper` names its start at once; before, the palette and the
/// shortcut said nothing, and no control shows the watch state.
#[test]
fn the_restart_names_its_start() {
    let notice = parts(restarting_notice());
    assert_eq!(notice.message_key, RESTARTING_KEY);
    assert_eq!(notice.level, NotificationLevel::Info);
    assert_eq!(notice.settings_key.as_deref(), Some(SETTINGS_KEY));
}

/// The end once the watch settled: the helper again, or what took its place.
#[test]
fn the_restart_names_its_end_with_the_watch_it_settled_on() {
    let helper = parts(restarted_notice(&info(WatcherStatus::Helper {
        distro: "debian".to_owned(),
        folders: 3,
    })));
    assert_eq!(helper.message_key, RESTARTED_KEY);
    assert_eq!(helper.level, NotificationLevel::Success);
    assert!(helper.args.contains_key("state"));
    let polling = parts(restarted_notice(&info(WatcherStatus::Polling {
        folders: 3,
        reason_key: Some("wsl.error.noHelper".to_owned()),
    })));
    assert_eq!(polling.level, NotificationLevel::Warning);
    let failed = parts(restarted_notice(&info(WatcherStatus::Failed {
        reason_key: "wsl.error.noHelper".to_owned(),
        detail: "gone".to_owned(),
    })));
    assert_eq!(failed.level, NotificationLevel::Error);
}

#[test]
fn only_a_started_watch_is_settled() {
    assert!(!is_settled(&WatcherStatus::Starting));
    assert!(is_settled(&WatcherStatus::Off));
    assert!(is_settled(&WatcherStatus::Native { folders: 1 }));
}
