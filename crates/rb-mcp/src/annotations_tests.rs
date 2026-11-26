use super::{DESTRUCTIVE, Hints, hints, named, of};
use crate::RbMcpServer;

/// What a client reads from `annotations`, MCP's defaults filled in (readOnlyHint and
/// idempotentHint false, destructiveHint and openWorldHint true; the latter two meaningless for
/// a read).
fn read_back(tool: &str) -> Hints {
    let annotations = of(tool);
    let read_only = annotations.read_only_hint.unwrap_or(false);
    Hints {
        read_only,
        destructive: !read_only && annotations.destructive_hint.unwrap_or(true),
        idempotent: read_only || annotations.idempotent_hint.unwrap_or(false),
        open_world: annotations.open_world_hint.unwrap_or(true),
    }
}

#[test]
fn every_core_tool_is_annotated_and_reads_back_as_its_hints() {
    let tools = RbMcpServer::router().list_all();
    assert!(!tools.is_empty());
    for tool in tools {
        let name = tool.name.as_ref();
        let annotations = tool
            .annotations
            .unwrap_or_else(|| panic!("{name} carries no annotations"));
        assert!(
            annotations.open_world_hint.is_some(),
            "{name}: openWorldHint"
        );
        assert!(
            annotations.read_only_hint == Some(true) || annotations.destructive_hint.is_some(),
            "{name}: a tool that changes something says whether it destroys"
        );
        assert_eq!(read_back(name), hints(name), "{name}");
    }
}

#[test]
fn the_lists_name_registered_tools_only() {
    let served = RbMcpServer::tool_names();
    for (list, tool) in named() {
        assert!(
            served.iter().any(|name| name == tool),
            "{list} names {tool}, which is not registered"
        );
    }
}

#[test]
fn a_read_is_neither_destructive_nor_listed_so() {
    for tool in RbMcpServer::tool_names() {
        let hints = hints(&tool);
        if hints.read_only {
            assert!(!hints.destructive, "{tool}");
            assert!(hints.idempotent, "{tool}");
            assert!(!DESTRUCTIVE.contains(&tool.as_str()), "{tool}");
        }
    }
}

#[test]
fn spot_checks() {
    let views_list = hints("views_list");
    assert!(views_list.read_only);
    assert!(!views_list.open_world);

    for installs in ["update_install", "extension_install"] {
        assert!(hints(installs).destructive, "{installs}");
    }

    let session_clear = hints("session_clear");
    assert!(!session_clear.read_only);
    assert!(session_clear.destructive);

    let navigate = hints("navigate");
    assert!(!navigate.read_only);
    assert!(!navigate.destructive);
    assert!(navigate.open_world);

    let config_set = hints("config_set");
    assert!(config_set.destructive);
    assert!(config_set.idempotent);

    let click = hints("click");
    assert!(!click.read_only);
    assert!(!click.idempotent);
}

#[test]
fn the_annotations_reach_the_wire_in_camel_case_without_defaults() {
    let wire = |tool: &str| serde_json::to_value(of(tool)).expect("annotations serialise");
    assert_eq!(
        wire("views_list"),
        serde_json::json!({ "readOnlyHint": true, "openWorldHint": false })
    );
    assert_eq!(
        wire("navigate"),
        serde_json::json!({ "destructiveHint": false, "openWorldHint": true })
    );
    assert_eq!(
        wire("session_clear"),
        serde_json::json!({ "destructiveHint": true, "idempotentHint": true,
                            "openWorldHint": false })
    );
}
