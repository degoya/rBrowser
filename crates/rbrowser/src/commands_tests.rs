use super::*;

#[test]
fn every_command_has_a_leader_chord_and_an_mcp_entry() {
    let definitions = definitions().expect("valid ids");
    let ids: Vec<&str> = definitions.iter().map(|def| def.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "view.zoom_fit_all",
            "view.zoom_in",
            "view.zoom_out",
            "view.focus_next",
            "view.focus_prev",
            "app.reload_config"
        ]
    );
    for def in &definitions {
        assert!(matches!(def.default_bindings[0], Binding::Leader(_)));
        assert_eq!(def.title_key, format!("commands.{}", def.id.as_str()));
        assert!(
            matches!(&def.mcp, McpCoverage::Omitted(reason) if reason.starts_with("ui-only")),
            "{:?}",
            def.mcp
        );
    }
    assert_eq!(
        definitions[0].default_bindings[0].to_string(),
        "Ctrl+Shift+Space V Z"
    );
}

#[test]
fn no_two_commands_share_a_chord() {
    let definitions = definitions().expect("valid ids");
    let mut chords: Vec<String> = definitions
        .iter()
        .flat_map(|def| def.default_bindings.iter().map(ToString::to_string))
        .collect();
    let count = chords.len();
    chords.sort();
    chords.dedup();
    assert_eq!(chords.len(), count);
}
