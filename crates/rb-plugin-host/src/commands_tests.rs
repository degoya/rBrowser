use rb_commands::{Binding, Group, McpCoverage};
use rb_plugin_types::codes;

use super::{
    CORE_COMMANDS, PANELS_COMMAND, PANELS_REASON, PROTECTED_REASON, core_command_defs,
    plugin_command_defs,
};
use crate::manifest::parse_manifest;
use crate::test_support::{ManifestBuilder, signing_key};

#[test]
fn only_plugins_list_reaches_mcp_and_everything_else_is_protected() {
    let defs = core_command_defs().expect("valid ids");
    assert_eq!(defs.len(), CORE_COMMANDS.len() + 1, "and plugins.panels");
    for def in &defs {
        assert_eq!(def.group, Group::Plugins);
        assert!(
            matches!(def.default_bindings[..], [Binding::Leader(_)]),
            "{}",
            def.id
        );
        match def.id.as_str() {
            "plugins.list" => assert_eq!(def.mcp, McpCoverage::Covered("plugins_list".to_owned())),
            PANELS_COMMAND => {
                assert_eq!(def.mcp, McpCoverage::Omitted(PANELS_REASON.to_owned()));
                assert_eq!(def.default_bindings[0].to_string(), "Ctrl+Shift+Space X P");
            }
            other => assert_eq!(
                def.mcp,
                McpCoverage::Omitted(PROTECTED_REASON.to_owned()),
                "{other}"
            ),
        }
    }
    let mut keys: Vec<String> = defs
        .iter()
        .map(|def| def.default_bindings[0].to_string())
        .collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), defs.len(), "every chord is its own");
}

#[test]
fn a_plugins_commands_are_namespaced_and_map_to_its_tools() {
    let builder = ManifestBuilder::named("breakpoint-analyzer", "breakpoints").tables(
        "\n[[tools]]\nname = \"detect\"\ndescription = \"plugin.breakpoint-analyzer.tools.detect\"\n\
         \n[[commands]]\nid = \"detect\"\ntitle = \"plugin.breakpoint-analyzer.commands.detect\"\ntool = \"detect\"\n\
         \n[[commands]]\nid = \"panel\"\ntitle = \"plugin.breakpoint-analyzer.commands.panel\"\nmcp_omitted = \"ui-only: opens a panel\"\ngroup = \"panels\"\n",
    );
    let manifest = parse_manifest(builder.text(&signing_key(1)).as_bytes()).expect("valid");
    let (defs, notices) = plugin_command_defs(&manifest);
    assert!(notices.is_empty());
    assert_eq!(defs[0].id.as_str(), "plugin.breakpoint_analyzer.detect");
    assert_eq!(
        defs[0].mcp,
        McpCoverage::Covered("breakpoints_detect".to_owned())
    );
    assert_eq!(defs[0].group, Group::Plugins);
    assert_eq!(defs[1].group, Group::Panels);
    assert!(matches!(defs[1].mcp, McpCoverage::Omitted(_)));
}

#[test]
fn a_reserved_default_shortcut_is_noticed_and_left_to_the_registry_to_drop() {
    let builder = ManifestBuilder::default().tables(
        "\n[[commands]]\nid = \"run\"\ntitle = \"plugin.sample.commands.run\"\nmcp_omitted = \"ui-only\"\nshortcut = \"Ctrl+T\"\n",
    );
    let manifest = parse_manifest(builder.text(&signing_key(1)).as_bytes()).expect("valid");
    let (defs, notices) = plugin_command_defs(&manifest);
    // Handed on, so the registry drops it with the notice the person sees.
    assert_eq!(defs[0].default_bindings.len(), 1);
    assert_eq!(notices[0].code(), Some(codes::SHORTCUT_UNBOUND));
    assert_eq!(notices[0].param("command"), Some("plugin.sample.run"));
}
