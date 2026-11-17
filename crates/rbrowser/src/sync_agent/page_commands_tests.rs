use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};

use super::*;

/// Every core command the app registers besides these, the other modules' included.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::breakpoint_keys::definitions().expect("the breakpoint shortcuts"));
    defs.extend(crate::sidecar::definitions().expect("the sidecar's"));
    defs.push(rb_diag::command_def().expect("diagnostics.create"));
    defs.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    defs
}

#[test]
fn the_toggles_pass_the_lint_against_the_reserved_list_and_the_others() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn the_toggles_have_their_sync_chords_and_name_the_settings_tools() {
    let defs = definitions().expect("definitions");
    let found: Vec<(String, Vec<String>)> = defs
        .iter()
        .map(|def| {
            (
                def.id.as_str().to_owned(),
                def.default_bindings
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            (
                TOGGLE_BUS.to_owned(),
                vec!["Ctrl+Shift+Space S B".to_owned()]
            ),
            (
                TOGGLE_API.to_owned(),
                vec!["Ctrl+Shift+Space S A".to_owned()]
            ),
            (
                TOGGLE_UA_SUFFIX.to_owned(),
                vec!["Ctrl+Shift+Space S I".to_owned()]
            ),
        ]
    );
    assert!(defs.iter().all(|def| def.group == Group::Sync
        && def.mcp == McpCoverage::Omitted(TOGGLES_OMITTED.to_owned())));
}

#[test]
fn each_toggle_names_its_switch_notice_and_setting() {
    assert_eq!(switch_of(TOGGLE_BUS), Some(Switch::Bus));
    assert_eq!(switch_of("sync.toggle"), None);
    assert_eq!(
        notice(Switch::Bus, false).message_key,
        "page.notice.bus_off"
    );
    let suffix = notice(Switch::UaSuffix, true);
    assert_eq!(suffix.message_key, "page.notice.ua_suffix_on");
    assert_eq!(
        suffix.args.get("version").map(String::as_str),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(suffix.settings_key.as_deref(), Some("pageApi.uaSuffix"));
    assert_eq!(
        notice(Switch::Api, true).settings_key.as_deref(),
        Some("pageApi.enabled")
    );
}
