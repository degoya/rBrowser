use rb_commands::{Binding, Chord, Group, Key, McpCoverage};

use super::*;

#[test]
fn export_and_import_are_leader_e_w_and_e_i_and_never_mcps() {
    let defs = definitions().expect("valid ids");
    let found: Vec<(&str, Vec<Binding>)> = defs
        .iter()
        .map(|def| (def.id.as_str(), def.default_bindings.clone()))
        .collect();
    let leader = |a: char, b: char| {
        vec![Binding::Leader(Chord {
            keys: vec![Key::Char(a), Key::Char(b)],
        })]
    };
    assert_eq!(
        found,
        [(EXPORT, leader('E', 'W')), (IMPORT, leader('E', 'I'))]
    );
    for def in &defs {
        assert_eq!(def.group, Group::Environments);
        assert_eq!(def.title_key, format!("commands.{}", def.id.as_str()));
        assert!(
            matches!(&def.mcp, McpCoverage::Omitted(reason) if reason.starts_with("secrets")),
            "{:?}",
            def.mcp
        );
    }
}

#[test]
fn mcp_is_refused_both_commands() {
    assert!(refuse_mcp(EXPORT).is_err());
    assert!(refuse_mcp(IMPORT).is_err());
    assert!(refuse_mcp("session.clear").is_ok());
}
