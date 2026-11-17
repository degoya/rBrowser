use super::*;

#[test]
fn command_ids_are_dotted_lower_case() {
    for good in ["view.set_breakpoint.2", "sync.toggle", "plugin.seo.check"] {
        assert!(CommandId::new(good).is_ok(), "{good}");
    }
    for bad in [
        "",
        "view",
        "View.zoom",
        "view..zoom",
        "view.zoom in",
        ".view",
    ] {
        assert_eq!(
            CommandId::new(bad),
            Err(CommandError::InvalidId(bad.to_owned())),
            "{bad}"
        );
    }
    let id = CommandId::new("view.set_breakpoint.2").expect("valid");
    assert_eq!(id.as_str(), "view.set_breakpoint.2");
    assert_eq!(
        serde_json::to_value(&id).expect("serializes"),
        "view.set_breakpoint.2"
    );
}

#[test]
fn the_fourteen_leader_groups() {
    let keys: String = Group::ALL.iter().map(|g| g.leader_key()).collect();
    assert_eq!(keys, "BLSEDOCKRPFVX,");
    assert_eq!(Group::Settings.id(), "settings");
    assert_eq!(Group::Breakpoints.title_key(), "commands.group.breakpoints");
}

#[test]
fn mcp_coverage_serializes_tagged() {
    let covered = McpCoverage::Covered("view_set_breakpoint".into());
    assert_eq!(
        serde_json::to_value(&covered).expect("serializes"),
        serde_json::json!({ "covered": "view_set_breakpoint" })
    );
}
