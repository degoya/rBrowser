use super::*;
use crate::{CommandRegistry, Handler};

#[test]
fn every_definition_registers() {
    let mut registry = CommandRegistry::new();
    for def in definitions() {
        let handler: Handler = std::sync::Arc::new(|_args| Ok(serde_json::Value::Null));
        let id = def.id.to_string();
        registry.register(def, handler).expect(&id);
    }
    assert_eq!(
        registry.iter().count(),
        ROWS.len() + crate::builtin_features::definitions().len()
    );
}

#[test]
fn the_shell_commands_are_defined_with_their_mcp_mapping() {
    let defs = definitions();
    let mcp = |id: &str| {
        defs.iter()
            .find(|def| def.id.as_str() == id)
            .map(|def| def.mcp.clone())
            .expect(id)
    };
    assert_eq!(mcp("view.back"), McpCoverage::Covered("back".into()));
    assert_eq!(mcp("view.forward"), McpCoverage::Covered("forward".into()));
    assert_eq!(mcp("reload.all"), McpCoverage::Covered("reload".into()));
    assert_eq!(
        mcp("view.navigate"),
        McpCoverage::Covered("navigate".into())
    );
    assert_eq!(mcp("sync.toggle"), McpCoverage::Covered("sync_set".into()));
    assert_eq!(
        mcp("layout.select_preset"),
        McpCoverage::Covered("layout_set".into())
    );
    assert_eq!(
        mcp("environments.select"),
        McpCoverage::Covered("environment_set".into())
    );
    assert_eq!(
        mcp("breakpoints.analyze"),
        McpCoverage::Covered("breakpoints_detect".into())
    );
    assert_eq!(mcp("settings.open"), McpCoverage::Omitted("ui-only".into()));
    assert_eq!(
        mcp("view.focus_address_bar"),
        McpCoverage::Omitted("ui-only".into())
    );
    assert_eq!(
        mcp("shortcuts.edit"),
        McpCoverage::Covered("settings_get".into())
    );
}

#[test]
fn sync_toggle_is_leader_s_s() {
    let defs = definitions();
    let sync = defs
        .iter()
        .find(|def| def.id.as_str() == "sync.toggle")
        .expect("defined");
    assert_eq!(sync.title_key, "commands.sync.toggle");
    assert_eq!(
        sync.default_bindings,
        vec!["Ctrl+Shift+Space S S".parse::<Binding>().expect("parses")]
    );
}

#[test]
fn every_title_has_its_english_text() {
    // The catalogue and the wave parts not merged yet (`locales/parts/*.json`), as the i18n lint
    // reads them: a job's branch passes with its titles in its part.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales");
    let mut catalogues: Vec<serde_json::Value> = vec![
        serde_json::from_str(include_str!("../../../locales/en.json"))
            .expect("the catalogue parses"),
    ];
    if let Ok(parts) = std::fs::read_dir(root.join("parts")) {
        for part in parts.flatten() {
            if part.path().extension().is_some_and(|ext| ext == "json") {
                let text = std::fs::read_to_string(part.path()).expect("the part reads");
                catalogues.push(serde_json::from_str(&text).expect("the part parses"));
            }
        }
    }
    let lookup = |key: &str| {
        catalogues.iter().find_map(|catalogue| {
            key.split('.')
                .try_fold(catalogue, |node, segment| node.get(segment))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
    };
    for def in definitions() {
        assert!(lookup(&def.title_key).is_some(), "{}", def.title_key);
    }
    for group in crate::Group::ALL {
        assert!(
            lookup(&group.title_key()).is_some(),
            "{}",
            group.title_key()
        );
    }
    for key in [
        crate::validate::REJECTED_LEADER,
        crate::validate::REJECTED_CONFLICT,
        crate::validate::REJECTED_INVALID,
        crate::registry::NOTICE_BINDING_DROPPED,
        crate::registry::NOTICE_LEADER_ASSIGNED,
        crate::registry::NOTICE_PALETTE_ONLY,
        crate::registry::NOTICE_UNKNOWN_COMMAND,
        crate::registry::NOTICE_USER_BINDING_REFUSED,
    ] {
        assert!(lookup(key).is_some(), "{key}");
    }
}
