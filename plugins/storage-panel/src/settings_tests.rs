use rb_plugin_api::test_support::MemoryHost;

use super::*;

#[test]
fn the_defaults_hold_without_settings() {
    let mut host = MemoryHost::new();
    assert_eq!(Settings::read(&mut host), Settings::default());
    assert!(
        !Settings::default().mcp_values,
        "MCP gets no values unless a person allows it"
    );
}

#[test]
fn the_user_values_are_read_and_bad_ones_fall_back() {
    let mut host = MemoryHost::new();
    host.settings.insert("pollMs".into(), "500".into());
    host.settings.insert("showHttpOnly".into(), "false".into());
    host.settings.insert("pinsEnabled".into(), "false".into());
    host.settings.insert("mcpValues".into(), "true".into());
    let settings = Settings::read(&mut host);
    assert_eq!(settings.poll_ms, 500);
    assert!(!settings.show_http_only && !settings.pins_enabled && settings.mcp_values);

    host.settings.insert("pollMs".into(), "600001".into());
    host.settings.insert("mcpValues".into(), "\"yes\"".into());
    let settings = Settings::read(&mut host);
    assert_eq!(settings.poll_ms, DEFAULT_POLL_MS);
    assert!(!settings.mcp_values);
}
