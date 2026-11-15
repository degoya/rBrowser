use serde_json::json;

use super::{DEFAULT_TIMEOUT_MS, Settings};
use crate::fake_host::FakeHost;
use crate::header::World;

#[test]
fn without_settings_the_defaults_hold() {
    let mut host = FakeHost::with_views(Vec::new());
    assert_eq!(Settings::read(&mut host), Settings::default());
    assert_eq!(Settings::default().default_timeout_ms, DEFAULT_TIMEOUT_MS);
    assert_eq!(Settings::default().default_world, World::Isolated);
    assert!(Settings::default().project_enabled);
}

#[test]
fn the_settings_read_and_bad_values_keep_the_default() {
    let mut host = FakeHost::with_views(Vec::new());
    host.set("defaultWorld", "\"main\"");
    host.set("defaultTimeoutMs", "2000");
    host.set("projectEnabled", "false");
    host.set(
        "library",
        &json!([{ "name": "a", "code": "1" }]).to_string(),
    );
    let settings = Settings::read(&mut host);
    assert_eq!(settings.default_world, World::Main);
    assert_eq!(settings.default_timeout_ms, 2000);
    assert!(!settings.project_enabled);
    assert_eq!(settings.library.len(), 1);
    host.set("defaultWorld", "\"page\"");
    host.set("defaultTimeoutMs", "60000");
    host.set("library", "{}");
    let settings = Settings::read(&mut host);
    assert_eq!(settings.default_world, World::Isolated);
    assert_eq!(settings.default_timeout_ms, DEFAULT_TIMEOUT_MS);
    assert!(settings.library.is_empty());
}
