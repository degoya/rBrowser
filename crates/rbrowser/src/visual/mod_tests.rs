use super::*;

#[test]
fn the_options_follow_the_config_and_the_threshold_argument() {
    let config = rb_config::VisualConfig {
        threshold: 0.2,
        full_page: false,
        slug_strategy: rb_config::SlugStrategy::HostPath,
        lfs_check: false,
        fail_on_meta_mismatch: true,
    };
    let options = options(&config, None);
    assert_eq!(options.threshold, 0.2);
    assert!(!options.full_page && !options.lfs_check && options.fail_on_meta_mismatch);
    assert_eq!(options.slug, SlugStrategy::HostPath);
    assert_eq!(super::options(&config, Some(0.05)).threshold, 0.05);
}

#[test]
fn the_defaults_compare_full_pages_and_check_lfs() {
    let options = options(&rb_config::VisualConfig::default(), None);
    assert_eq!(options.threshold, 0.1);
    assert!(options.full_page && options.lfs_check && !options.fail_on_meta_mismatch);
    assert_eq!(options.slug, SlugStrategy::Path);
}

#[test]
fn the_machine_meta_names_this_build() {
    let meta = machine_meta(1_700_000_000);
    assert_eq!(meta.os, std::env::consts::OS);
    assert_eq!(meta.app_version, BUILD.version);
    assert_eq!(meta.created_at, 1_700_000_000);
    assert!(!meta.machine.is_empty() && !meta.engine_version.is_empty());
}

#[test]
fn the_commands_are_this_modules_only() {
    assert_eq!(
        COMMANDS,
        [
            "visual.compare",
            "visual.baseline_update",
            "visual.lfs_setup"
        ]
    );
    for key in [
        COMPARED_KEY,
        UPDATED_KEY,
        LFS_MISSING_KEY,
        LFS_DONE_KEY,
        BUSY_KEY,
    ] {
        assert!(key.starts_with("visual."), "{key}");
    }
}

/// RB-090-85: a compare and a baseline update name their start with the number of views; the
/// outcome follows as `visual.notice.compared` or `visual.notice.updated`.
#[test]
fn a_started_run_names_itself_with_its_views() {
    for key in [COMPARING_KEY, UPDATING_KEY] {
        let CoreToShell::Notification(notification) = started_notice(key, 3) else {
            panic!("a notification");
        };
        assert_eq!(notification.message_key, key);
        assert_eq!(notification.level, NotificationLevel::Info);
        assert_eq!(notification.args["count"], "3");
        assert_eq!(notification.settings_key.as_deref(), Some(SETTINGS_KEY));
    }
}
