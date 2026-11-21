use rb_plugin_api::test_support::MemoryHost;

use super::*;

#[test]
fn the_defaults_reject_and_allow_nothing() {
    let settings = Settings::read(&mut MemoryHost::new());
    assert!(settings.reject_step);
    assert!(settings.allow_hosts.is_empty());
    assert!(settings.essential_cookies.is_empty());
}

#[test]
fn the_settings_are_read_and_held_to_their_bounds() {
    let mut host = MemoryHost::new();
    host.settings.insert(
        "allowHosts".to_owned(),
        r#"["cdn.tracker.test", "*.assets.test", 7, "bad host"]"#.to_owned(),
    );
    host.settings
        .insert("rejectStep".to_owned(), "false".to_owned());
    let many: Vec<String> = (0..100).map(|n| format!("c{n}")).collect();
    let mut list = vec![
        "shop_cart".to_owned(),
        "pref_*".to_owned(),
        "a=b".to_owned(),
    ];
    list.extend(many);
    host.settings.insert(
        "essentialCookies".to_owned(),
        serde_json::to_string(&list).expect("JSON"),
    );
    let settings = Settings::read(&mut host);
    assert!(!settings.reject_step);
    assert_eq!(settings.allow_hosts.len(), 2);
    assert_eq!(settings.essential_cookies.len(), MAX_COOKIE_NAMES);
    assert_eq!(settings.essential_cookies[0], "shop_cart");
    assert_eq!(settings.essential_cookies[1], "pref_*");
    assert!(!settings.essential_cookies.contains(&"a=b".to_owned()));
}

#[test]
fn a_setting_of_the_wrong_type_keeps_the_default() {
    let mut host = MemoryHost::new();
    host.settings
        .insert("rejectStep".to_owned(), "\"no\"".to_owned());
    host.settings
        .insert("allowHosts".to_owned(), "\"x.test\"".to_owned());
    let settings = Settings::read(&mut host);
    assert!(settings.reject_step);
    assert!(settings.allow_hosts.is_empty());
}
