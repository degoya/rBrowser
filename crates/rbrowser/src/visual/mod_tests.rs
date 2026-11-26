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
            "visual.compare_visible",
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

fn host() -> Arc<Host> {
    let (_fake, router) = crate::cdp::fake::FakeBrowser::connect();
    let ui = Arc::new(crate::host::ui::recording::RecordingUi::default());
    let host = Host::new(ui, router, tokio::runtime::Handle::current());
    host.open_config(crate::host::state::default_config(), None)
        .expect("opens");
    host
}

/// RB-099-10: a scope takes the views of its breakpoints only; the others are not captured, so
/// their baselines stay as they are.
#[tokio::test]
async fn a_scope_takes_only_the_views_of_its_breakpoints() {
    let host = host();
    let chosen = ViewScope::Breakpoints(vec!["tablet".to_owned()]);
    let (views, not_shown) = targets(&host, Target::All, Some(&chosen)).expect("one view");
    let names: Vec<&str> = views.iter().map(|view| view.name.as_str()).collect();
    assert_eq!(names, ["tablet"]);
    assert!(not_shown.is_empty());
    let (every, _) = targets(&host, Target::All, None).expect("every view");
    assert_eq!(every.len(), host.views().len());
    let (visible, _) = targets(&host, Target::All, Some(&ViewScope::Visible)).expect("visible");
    assert_eq!(visible.len(), every.len());
    let refused = targets(
        &host,
        Target::All,
        Some(&ViewScope::Breakpoints(Vec::new())),
    );
    assert!(
        matches!(refused, Err(VisualRunError::Scope(_))),
        "{:?}",
        refused.map(|(views, _)| views.len())
    );
}
