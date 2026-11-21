use rb_plugin_api::test_support::MemoryHost;

use super::*;

#[test]
fn the_defaults_check_rich_results_and_show_every_severity() {
    let mut host = MemoryHost::new();
    assert_eq!(Settings::read(&mut host), Settings::default());
    assert!(Settings::default().rich);
    assert_eq!(Settings::default().min_severity, Severity::Info);
}

#[test]
fn the_keys_are_read_and_a_malformed_one_keeps_its_default() {
    let mut host = MemoryHost::new();
    host.settings
        .insert("richResults".to_owned(), "false".to_owned());
    host.settings
        .insert("minSeverity".to_owned(), "\"warning\"".to_owned());
    assert_eq!(
        Settings::read(&mut host),
        Settings {
            rich: false,
            min_severity: Severity::Warning
        }
    );
    host.settings
        .insert("richResults".to_owned(), "\"no\"".to_owned());
    host.settings
        .insert("minSeverity".to_owned(), "\"fatal\"".to_owned());
    assert_eq!(Settings::read(&mut host), Settings::default());
}
