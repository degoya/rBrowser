use rb_commands::McpCoverage;

use super::*;

#[test]
fn the_commands_have_their_chords_and_their_mcp_entries() {
    let defs = definitions().expect("valid ids");
    let ids: Vec<&str> = defs.iter().map(|def| def.id.as_str()).collect();
    assert_eq!(ids, [START, NEW_LINK, STOP]);
    let chords: Vec<String> = defs
        .iter()
        .flat_map(|def| def.default_bindings.iter().map(ToString::to_string))
        .collect();
    assert_eq!(
        chords,
        [
            "Ctrl+Shift+Space S W",
            "Ctrl+Shift+Space S K",
            "Ctrl+Shift+Space S E"
        ]
    );
    // Sharing the site on the internet is a person's act; MCP may only end it.
    assert_eq!(defs[0].mcp, McpCoverage::Omitted(START_OMITTED.to_owned()));
    assert_eq!(
        defs[1].mcp,
        McpCoverage::Omitted(NEW_LINK_OMITTED.to_owned())
    );
    assert_eq!(
        defs[2].mcp,
        McpCoverage::Covered("portal_present_stop".to_owned())
    );
    for def in &defs {
        assert_eq!(def.title_key, format!("commands.{}", def.id.as_str()));
    }
    let mut registry = CommandRegistry::new();
    for def in defs {
        let handler: Handler = Arc::new(|_| Ok(Value::Null));
        registry.register(def, handler).expect("registered");
    }
}

/// The chords are free beside the portal's own and every other `Leader S` command.
#[test]
fn the_chords_are_free_in_the_sync_group() {
    let taken: Vec<String> = crate::portal::definitions()
        .expect("valid ids")
        .iter()
        .flat_map(|def| def.default_bindings.iter().map(ToString::to_string))
        .collect();
    for def in definitions().expect("valid ids") {
        for binding in &def.default_bindings {
            assert!(!taken.contains(&binding.to_string()), "{binding}");
        }
    }
}
