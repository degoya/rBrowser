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
