use rb_plugin_api::test_support::MemoryHost;

use super::*;

#[test]
fn defaults_hold_without_settings() {
    let mut host = MemoryHost::new();
    assert_eq!(Settings::read(&mut host), Settings::default());
    assert!(Settings::default().offline);
    assert_eq!(Settings::default().timeout_ms, 30_000);
}

#[test]
fn values_are_read_and_held_to_the_schema_bounds() {
    let mut host = MemoryHost::new();
    host.settings
        .insert("offline.enabled".to_owned(), "false".to_owned());
    host.settings
        .insert("timeoutMs".to_owned(), "999999".to_owned());
    let settings = Settings::read(&mut host);
    assert!(!settings.offline);
    assert_eq!(settings.timeout_ms, MAX_TIMEOUT_MS);
    host.settings.insert("timeoutMs".to_owned(), "1".to_owned());
    assert_eq!(Settings::read(&mut host).timeout_ms, MIN_TIMEOUT_MS);
    host.settings
        .insert("timeoutMs".to_owned(), "\"soon\"".to_owned());
    assert_eq!(Settings::read(&mut host).timeout_ms, 30_000);
}
