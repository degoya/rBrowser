use rb_commands::{Binding, Group, Key, McpCoverage};

use super::*;

#[test]
fn environment_auth_is_leader_e_a_and_never_mcps() {
    let defs = definitions().expect("valid ids");
    assert_eq!(defs.len(), 1);
    let def = &defs[0];
    assert_eq!(def.id.as_str(), AUTH);
    assert_eq!(def.title_key, "commands.environment.auth");
    assert_eq!(def.group, Group::Environments);
    assert_eq!(
        def.default_bindings,
        vec![Binding::Leader(rb_commands::Chord {
            keys: vec![Key::Char('E'), Key::Char('A')],
        })]
    );
    assert!(
        matches!(&def.mcp, McpCoverage::Omitted(reason) if reason.starts_with("secrets")),
        "{:?}",
        def.mcp
    );
}

#[test]
fn mcp_is_refused_the_command_even_by_name() {
    assert!(refuse_mcp(AUTH).is_err());
    assert!(refuse_mcp("session.assign").is_ok());
}
