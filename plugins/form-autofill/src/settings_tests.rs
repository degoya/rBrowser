use super::*;
use crate::fake::FakeHost;

#[test]
fn defaults_without_settings() {
    let settings = Settings::read(&mut FakeHost::default());
    assert_eq!(settings, Settings::default());
    assert_eq!(settings.strategy, Strategy::Mixed);
    assert!(settings.skip_hidden);
    assert_eq!(settings.seed, 0);
}

#[test]
fn the_settings_are_read_and_bad_values_fall_back() {
    let mut host = FakeHost::default();
    host.set("invalidStrategy", "\"bounds\"")
        .set("seed", "12345")
        .set("skipHidden", "false")
        .set("activeProfile", "\"testuser\"")
        .set(
            "profiles",
            r#"[{"name":"testuser","values":{"email":"qa@example.com","n":7}},{"values":{}},"x"]"#,
        );
    let settings = Settings::read(&mut host);
    assert_eq!(settings.strategy, Strategy::Bounds);
    assert_eq!(settings.seed, 12345);
    assert!(!settings.skip_hidden);
    assert_eq!(settings.active_profile, "testuser");
    assert_eq!(settings.profiles.len(), 1);
    let profile = settings.profile("testuser").expect("found");
    assert_eq!(
        profile.values.get("email").map(String::as_str),
        Some("qa@example.com")
    );
    assert!(!profile.values.contains_key("n"));

    host.set("invalidStrategy", "\"sideways\"")
        .set("seed", "-4");
    let settings = Settings::read(&mut host);
    assert_eq!(settings.strategy, Strategy::Mixed);
    assert_eq!(settings.seed, 0);
}

#[test]
fn the_setting_takes_every_seed_the_tool_takes() {
    // The tool `autofill_fill` and the panel take 0..=2^32-1; the setting the same range.
    let mut host = FakeHost::default();
    host.set("seed", "4294967295");
    assert_eq!(Settings::read(&mut host).seed, u64::from(u32::MAX));
    host.set("seed", "4294967296");
    assert_eq!(Settings::read(&mut host).seed, 0);
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../settings.schema.json")).expect("the schema");
    assert_eq!(schema["properties"]["seed"]["maximum"], u64::from(u32::MAX));
}
