use rb_plugin_api::test_support::MemoryHost;

use super::*;

fn with(value: &str) -> Settings {
    let mut host = MemoryHost::new();
    host.settings
        .insert("hstsMinAge".to_owned(), value.to_owned());
    Settings::read(&mut host)
}

#[test]
fn the_default_is_one_year() {
    assert_eq!(
        Settings::read(&mut MemoryHost::new()).hsts_min_age,
        31_536_000
    );
}

#[test]
fn a_valid_age_is_taken_and_anything_else_keeps_the_default() {
    assert_eq!(with("15552000").hsts_min_age, 15_552_000);
    assert_eq!(with("0").hsts_min_age, 0);
    for bad in ["-1", "\"year\"", "1.5", "99999999999", "not json"] {
        assert_eq!(with(bad).hsts_min_age, DEFAULT_HSTS_MIN_AGE, "{bad}");
    }
}
