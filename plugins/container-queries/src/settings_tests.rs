use rb_plugin_api::test_support::MemoryHost;

use super::*;

#[test]
fn the_defaults_hold_without_settings() {
    let mut host = MemoryHost::new();
    assert_eq!(page_settings(&mut host), PageSettings::default());
    assert_eq!(page_settings(&mut host).color, "#0d9488");
    assert!(page_settings(&mut host).labels);
}

#[test]
fn the_settings_are_read_under_either_name_and_checked() {
    let mut host = MemoryHost::new();
    host.settings
        .insert("overlay.labels".to_owned(), "false".to_owned());
    host.settings.insert(
        "container-queries.overlay.color".to_owned(),
        "\"#AB12cd\"".to_owned(),
    );
    assert_eq!(
        page_settings(&mut host),
        PageSettings {
            labels: false,
            color: "#AB12cd".to_owned()
        }
    );
    host.settings
        .insert("overlay.color".to_owned(), "\"red\"".to_owned());
    host.settings
        .insert("overlay.labels".to_owned(), "\"no\"".to_owned());
    assert_eq!(page_settings(&mut host), PageSettings::default());
}

#[test]
fn a_colour_is_hash_and_six_hex_digits() {
    assert!(is_color("#0d9488"));
    assert!(!is_color("#0d948"));
    assert!(!is_color("0d94888"));
    assert!(!is_color("#0d948g"));
}
