use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin, builtin_reserved};
use rb_config::UserSettings;
use serde_json::json;

use super::*;

use crate::host::ui::recording::{RecordingUi, UiCall};
use crate::layout::test_support::{config, setup};

/// Every core command with a leader chord in the View group besides this module's.
fn other_core_defs() -> Vec<CommandDef> {
    let mut defs = builtin::definitions();
    defs.extend(crate::commands::definitions().expect("the grid's definitions"));
    defs.extend(crate::breakpoint_keys::definitions().expect("breakpoint keys"));
    defs.extend(crate::emulation::extras::definitions().expect("emulation extras"));
    defs.extend(crate::layout::defs::definitions().expect("layout modes"));
    defs
}

#[test]
fn the_chords_pass_the_lint_against_the_reserved_list_and_the_view_group() {
    let mut defs = other_core_defs();
    defs.extend(definitions().expect("definitions"));
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[test]
fn the_commands_their_chords_and_tools() {
    let summary: Vec<(String, String, McpCoverage)> = definitions()
        .expect("definitions")
        .iter()
        .map(|def| {
            (
                def.id.as_str().to_owned(),
                def.default_bindings[0].to_string(),
                def.mcp.clone(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                POP_OUT.to_owned(),
                "Ctrl+Shift+Space V X".to_owned(),
                McpCoverage::Covered("view_pop_out".to_owned())
            ),
            (
                POP_IN.to_owned(),
                "Ctrl+Shift+Space V V".to_owned(),
                McpCoverage::Covered("view_pop_in".to_owned())
            ),
        ]
    );
}

fn popped(ui: &RecordingUi) -> Vec<crate::host::ui::PopOutWindow> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::PopOut(window) => Some(window),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn pop_out_takes_the_named_view_or_the_active_one_with_the_settings() {
    let setup = setup(config());
    let core = &setup.core;
    let mut user = UserSettings::default();
    user.pop_out.fit_to_screen = false;
    user.pop_out.keep_placeholder = false;
    core.replace_user_settings(user);
    let answer = run_with(core, POP_OUT, &json!({ "view": 2 }), None).expect("pops out");
    assert_eq!(
        answer,
        json!({ "view": 2, "detached": true, "changed": true })
    );
    let window = popped(&setup.ui).pop().expect("a window");
    assert_eq!(window.view, ViewId(2));
    assert!(!window.fit_to_screen);
    assert!(core.host().is_detached(ViewId(2)));
    // Without a view: the active one.
    core.host().set_active_view(ViewId(3)).expect("active");
    run_with(core, POP_OUT, &json!({}), None).expect("pops out");
    assert!(core.host().is_detached(ViewId(3)));
    // Back.
    let answer = run_with(core, POP_IN, &json!({ "view": 2 }), None).expect("pops in");
    assert_eq!(
        answer,
        json!({ "view": 2, "detached": false, "changed": true })
    );
    let answer = run_with(core, POP_IN, &json!({ "view": 2 }), None).expect("answers");
    assert_eq!(
        answer,
        json!({ "view": 2, "detached": false, "changed": false })
    );
}

#[tokio::test]
async fn remembered_bounds_go_with_the_window_only_when_kept() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(store::STATE_FILE);
    let mut state = store::PopOutState::default();
    let client = ScreenRect {
        x: 200,
        y: 120,
        width: 1000,
        height: 700,
    };
    state.remember("desktop", client);
    store::save(&path, &state).expect("saved");
    let setup = setup(config());
    let core = &setup.core;
    run_with(core, POP_OUT, &json!({ "view": 1 }), Some(&path)).expect("pops out");
    assert_eq!(
        popped(&setup.ui).pop().expect("window").remembered,
        Some(client)
    );
    run_with(core, POP_IN, &json!({ "view": 1 }), Some(&path)).expect("pops in");
    let mut user = UserSettings::default();
    user.pop_out.remember_bounds = false;
    core.replace_user_settings(user);
    run_with(core, POP_OUT, &json!({ "view": 1 }), Some(&path)).expect("pops out");
    assert_eq!(popped(&setup.ui).pop().expect("window").remembered, None);
}

#[tokio::test]
async fn a_closed_window_keeps_its_bounds_and_brings_the_view_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(store::STATE_FILE);
    let setup = setup(config());
    let core = &setup.core;
    run_with(core, POP_OUT, &json!({ "view": 1 }), Some(&path)).expect("pops out");
    let client = ScreenRect {
        x: 50,
        y: 60,
        width: 1300,
        height: 800,
    };
    window_closed(core, ViewId(1), Some(client), Some(&path));
    assert!(!core.host().is_detached(ViewId(1)));
    assert_eq!(store::load(&path).bounds("desktop"), Some(client));
}

#[tokio::test]
async fn bad_arguments_and_unknown_views_are_refused() {
    let setup = setup(config());
    let core = &setup.core;
    assert!(matches!(
        run_with(core, POP_OUT, &json!({ "view": "one" }), None),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        run_with(core, POP_OUT, &json!({ "view": 99 }), None),
        Err(CommandError::Failed(_))
    ));
}
