use super::*;
use crate::fake_host::FakeHost;

#[test]
fn defaults_without_settings() {
    let mut host = FakeHost::default();
    let settings = Settings::read(&mut host);
    assert_eq!(settings, Settings::default());
    assert!(!settings.levels.contains(&Level::Debug));
    assert!(!settings.mcp_exec);
    assert!(
        !settings.show_rbrowser,
        "rBrowser's own messages are off by default"
    );
    assert_eq!(settings.buffer_size, 1000);
}

#[test]
fn the_hosts_values_apply_and_bad_ones_fall_back() {
    let mut host = FakeHost::default();
    host.set("levels", r#"["error","network","bogus"]"#);
    host.set("bufferSize", "50");
    host.set("clearOnNavigate", "false");
    host.set("networkLevel", r#""all""#);
    host.set("execTarget", r#""active""#);
    host.set("mcpExec", "true");
    host.set("showRbrowserMessages", "true");
    let settings = Settings::read(&mut host);
    assert_eq!(
        settings.levels.into_iter().collect::<Vec<_>>(),
        [Level::Error, Level::Network]
    );
    assert_eq!(settings.buffer_size, 100, "clamped to the minimum");
    assert!(!settings.clear_on_navigate);
    assert_eq!(settings.network, NetworkLevel::All);
    assert_eq!(settings.exec_target, Target::Active);
    assert!(settings.mcp_exec);
    assert!(settings.show_rbrowser);

    let mut host = FakeHost::default();
    host.set("mcpExec", r#""yes""#);
    host.set("bufferSize", "not json");
    let settings = Settings::read(&mut host);
    assert!(!settings.mcp_exec, "only true allows");
    assert_eq!(settings.buffer_size, 1000);
}

#[test]
fn the_clear_mark_round_trips_through_the_out_folder() {
    let mut host = FakeHost::with_views(Vec::new());
    assert_eq!(cleared_at(&mut host), 0.0);
    let mark = clear(&mut host, 1_731_300_000_500.0);
    assert_eq!(mark, 1_731_300_000_500.0);
    assert_eq!(cleared_at(&mut host), mark);
    assert!(host.files.contains_key(STATE_PATH));
}

#[test]
fn a_mark_is_never_later_than_the_clock() {
    let mut host = FakeHost::with_views(Vec::new());
    let ceiling = (1_731_300_000.0 + 1.0) * 1000.0;
    assert_eq!(clear(&mut host, 9e15), ceiling);
    assert_eq!(clear(&mut host, f64::INFINITY), ceiling);
}

#[test]
fn a_mark_that_cannot_be_written_is_logged() {
    let mut host = FakeHost::with_views(Vec::new());
    host.out_fails = true;
    let mark = clear(&mut host, 5.0);
    assert_eq!(mark, 5.0);
    assert_eq!(cleared_at(&mut host), 0.0);
    assert_eq!(host.logs.len(), 1);
    assert!(host.logs[0].1.contains("plugins.error.not_available"));
}
