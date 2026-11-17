use rb_commands::{Binding, Group, Key, McpCoverage};

use super::*;

fn chord(def: &CommandDef) -> Vec<Key> {
    match def.default_bindings.as_slice() {
        [Binding::Leader(chord)] => chord.keys.clone(),
        other => panic!("one leader chord expected, got {other:?}"),
    }
}

#[test]
fn restart_is_leader_r_s_without_mcp_tool() {
    let def = restart_def().expect("valid");
    assert_eq!(def.id.as_str(), "sidecar.restart");
    assert_eq!(def.title_key, "commands.sidecar.restart");
    // `V S` is `view.set_size` (RB-010-16); the restart sits beside `wsl.restart_helper` (`R W`).
    assert_eq!(def.group, Group::Reload);
    assert_eq!(chord(&def), vec![Key::Char('R'), Key::Char('S')]);
    assert_eq!(def.mcp, McpCoverage::Omitted(RESTART_OMITTED.to_owned()));
}

#[test]
fn update_is_leader_v_u_without_mcp_tool() {
    let def = update_def().expect("valid");
    assert_eq!(def.id.as_str(), "engines.update");
    assert_eq!(def.title_key, "commands.engines.update");
    assert_eq!(def.group, Group::View);
    assert_eq!(chord(&def), vec![Key::Char('V'), Key::Char('U')]);
    assert_eq!(def.mcp, McpCoverage::Omitted(UPDATE_OMITTED.to_owned()));
}
