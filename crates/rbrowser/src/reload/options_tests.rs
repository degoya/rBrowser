use std::time::Duration;

use rb_config::{ReloadConfig, ReloadMode, UserReloadSettings};

use super::*;

#[test]
fn the_config_maps_onto_the_options() {
    let project = ReloadConfig {
        watch: vec!["src/**".into()],
        ignore: vec!["dist/".into()],
        debounce_ms: 120,
        mode: ReloadMode::Manual,
        respect_hmr: false,
        interval_sec: 30,
    };
    let user = UserReloadSettings {
        polling_fallback: true,
        poll_interval_ms: 500,
        interval_pause_sec: 3,
    };
    let environments = [
        rb_config::Environment {
            id: "staging".into(),
            url: "https://staging.example.com".into(),
            color: None,
            reload_interval_sec: Some(10),
        },
        rb_config::Environment {
            id: "local".into(),
            url: "http://localhost:3000".into(),
            color: None,
            reload_interval_sec: None,
        },
    ];
    let options = ReloadOptions::from_config(&project, &environments, &user);
    assert_eq!(options.mode, HotMode::Off);
    assert_eq!(options.watch, vec!["src/**".to_owned()]);
    assert_eq!(options.debounce_ms, 120);
    assert!(options.polling);
    assert!(!options.respect_hmr);
    assert_eq!(options.interval(None), Some(Duration::from_secs(30)));
    assert_eq!(
        options.interval(Some("staging")),
        Some(Duration::from_secs(10))
    );
    assert_eq!(
        options.interval(Some("local")),
        Some(Duration::from_secs(30))
    );
    assert_eq!(options.poll_interval(), Duration::from_millis(500));
    assert_eq!(options.interval_pause(), Duration::from_secs(3));
}

#[test]
fn the_full_mode_reloads_every_change() {
    assert_eq!(hot_mode(ReloadMode::Full), HotMode::Full);
}

#[test]
fn the_defaults_reload_automatically_after_200_ms() {
    let options = ReloadOptions::default();
    assert_eq!(options.mode, HotMode::Auto);
    assert_eq!(options.debounce_ms, 200);
    assert!(!options.polling);
    assert_eq!(options.poll_interval(), Duration::from_secs(1));
}

#[test]
fn the_rules_take_the_ignore_file_after_the_config() {
    let options = ReloadOptions {
        ignore: vec!["dist/".into()],
        ..ReloadOptions::default()
    };
    let rules = options.rules(Some("# generated\ncoverage/\n\n!dist/keep.css\n"));
    assert_eq!(rules.ignore, vec!["dist/", "coverage/", "!dist/keep.css"]);
    assert_eq!(rules.debounce_ms, 200);
    assert_eq!(options.rules(None).ignore, vec!["dist/"]);
}

#[test]
fn an_environment_interval_wins_over_the_global_one() {
    let mut options = ReloadOptions {
        interval_sec: 30,
        ..ReloadOptions::default()
    };
    options.environment_intervals.insert("staging".into(), 10);
    options.environment_intervals.insert("local".into(), 0);
    assert_eq!(
        options.interval(Some("staging")),
        Some(Duration::from_secs(10))
    );
    assert_eq!(
        options.interval(Some("prod")),
        Some(Duration::from_secs(30))
    );
    assert_eq!(options.interval(None), Some(Duration::from_secs(30)));
    assert_eq!(options.interval(Some("local")), None, "0 switches it off");
}

#[test]
fn intervals_have_a_floor() {
    let options = ReloadOptions {
        interval_sec: 1,
        poll_interval_ms: 10,
        ..ReloadOptions::default()
    };
    assert_eq!(options.interval(None), Some(Duration::from_secs(2)));
    assert_eq!(options.poll_interval(), Duration::from_millis(250));
}

#[test]
fn the_ignore_file_is_read_from_the_project() {
    let dir = tempfile::tempdir().expect("a temp dir");
    assert_eq!(read_ignore_file(dir.path()), None);
    std::fs::create_dir(dir.path().join(".rbrowser")).expect("a folder");
    std::fs::write(dir.path().join(".rbrowser/ignore"), "dist/\n").expect("written");
    assert_eq!(read_ignore_file(dir.path()).as_deref(), Some("dist/\n"));
}
