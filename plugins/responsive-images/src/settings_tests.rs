use super::*;
use crate::fake::FakeHost;

#[test]
fn the_defaults_hold_without_settings() {
    let mut host = FakeHost::default();
    assert_eq!(Settings::read(&mut host), Settings::default());
    let defaults = Settings::default();
    assert!((defaults.oversize_factor - 1.5).abs() < f64::EPSILON);
    assert!((defaults.undersize_factor - 1.0).abs() < f64::EPSILON);
    assert!(defaults.check_lcp);
}

#[test]
fn the_settings_are_read_and_held_to_their_ranges() {
    let mut host = FakeHost::default();
    host.settings
        .insert("oversizeFactor".to_owned(), "2.5".to_owned());
    host.settings
        .insert("undersizeFactor".to_owned(), "0.75".to_owned());
    host.settings
        .insert("checkLcp".to_owned(), "false".to_owned());
    assert_eq!(
        Settings::read(&mut host),
        Settings {
            oversize_factor: 2.5,
            undersize_factor: 0.75,
            check_lcp: false
        }
    );
    for (key, bad) in [
        ("oversizeFactor", "1.0"),
        ("oversizeFactor", "100"),
        ("undersizeFactor", "1.5"),
        ("undersizeFactor", "\"high\""),
        ("checkLcp", "1"),
    ] {
        let mut host = FakeHost::default();
        host.settings.insert(key.to_owned(), bad.to_owned());
        assert_eq!(
            Settings::read(&mut host),
            Settings::default(),
            "{key}={bad}"
        );
    }
}
