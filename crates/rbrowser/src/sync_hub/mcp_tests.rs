use serde_json::json;

use super::*;
use crate::sync_hub::on_start;
use crate::sync_hub::tests::setup;

#[test]
fn the_tool_is_named_as_the_commands_say() {
    assert_eq!(SYNC_SET_TOOL, "sync_set");
    for id in [
        "sync.toggle",
        "sync.skip_next",
        "sync.resync",
        "sync.toggle_view",
        "sync.toggle_focus",
    ] {
        let def = rb_commands::builtin::definitions()
            .into_iter()
            .find(|def| def.id.as_str() == id)
            .unwrap_or_else(|| panic!("{id} is defined"));
        assert_eq!(
            def.mcp,
            rb_commands::McpCoverage::Covered(SYNC_SET_TOOL.into()),
            "{id}"
        );
    }
}

#[test]
fn the_arguments_parse_with_their_defaults() {
    let args: SyncSetArgs = serde_json::from_value(json!({ "enabled": false })).expect("parses");
    assert_eq!(args.action, SyncAction::Set);
    assert_eq!(args.target, None);
    let args: SyncSetArgs =
        serde_json::from_value(json!({ "target": 2, "action": "resync" })).expect("parses");
    assert_eq!(args.target, Some(Target::View(ViewId(2))));
    assert_eq!(args.action, SyncAction::Resync);
    let args: SyncSetArgs =
        serde_json::from_value(json!({ "action": "skipNextClick", "target": "all" }))
            .expect("parses");
    assert_eq!(args.target, Some(Target::All));
    assert!(serde_json::from_value::<SyncSetArgs>(json!({ "token": "x" })).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_set_switches_sync_a_kind_or_a_view() {
    let setup = setup();
    on_start(&setup.core);
    let core = &setup.core;
    let state = sync_set(core, &json!({ "enabled": false })).expect("set");
    assert!(!state.enabled);
    let state = sync_set(
        core,
        &json!({ "feature": "hover", "enabled": false, "target": "all" }),
    )
    .expect("set");
    assert!(!state.features.hover && state.features.click);
    let state = sync_set(core, &json!({ "target": 3, "enabled": false })).expect("set");
    assert_eq!(state.excluded, vec![ViewId(3)]);
    let json = serde_json::to_value(&state).expect("serialises");
    assert_eq!(json["excluded"], json!([3]));
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_set_refuses_what_makes_no_sense() {
    let setup = setup();
    let core = &setup.core;
    assert!(matches!(
        sync_set(core, &json!({ "enabled": true })),
        Err(CommandError::Failed(_))
    ));
    on_start(core);
    for args in [
        json!({ "action": "set" }),
        json!({ "target": 2, "feature": "click", "enabled": false }),
        json!({ "target": 9, "enabled": false }),
        json!({ "action": "jump" }),
    ] {
        assert!(
            matches!(sync_set(core, &args), Err(CommandError::InvalidArgs(_))),
            "{args}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_set_resyncs_and_skips() {
    let setup = setup();
    on_start(&setup.core);
    let core = &setup.core;
    sync_set(core, &json!({ "action": "skipNextClick" })).expect("skips");
    let sync = for_host(core.host()).expect("started");
    assert!(sync.hub().with(|hub| hub.status().skip_next_click));
    let state = sync_set(core, &json!({ "action": "resync" })).expect("resyncs");
    assert!(state.desynced.is_empty());
    assert_eq!(view_desync(core.host(), ViewId(1)), None);
}

/// RB-020-05: focus and form sync through `feature`; the answer says the skip is armed.
#[tokio::test(flavor = "multi_thread")]
async fn sync_set_switches_focus_and_form_sync() {
    let setup = setup();
    on_start(&setup.core);
    let core = &setup.core;
    let state = sync_set(core, &json!({ "action": "set" , "enabled": true })).expect("set");
    assert!(state.features.forms && !state.features.focus && !state.skip_next_click);
    let state = sync_set(core, &json!({ "feature": "focus", "enabled": true })).expect("set");
    assert!(state.features.focus);
    let state = sync_set(core, &json!({ "feature": "forms", "enabled": false })).expect("set");
    assert!(!state.features.forms);
    let state = sync_set(core, &json!({ "action": "skipNextClick" })).expect("skips");
    assert!(state.skip_next_click);
    let json = serde_json::to_value(&state).expect("serialises");
    assert_eq!(json["skipNextClick"], true);
    assert_eq!(json["features"]["focus"], true);
}

/// RB-090-33: the agents follow a scroll mode set at runtime - wheel mode has them capture the
/// wheel, the project's `sync.*` reloaded go back to its mode.
#[tokio::test(flavor = "multi_thread")]
async fn sync_set_scroll_mode_reaches_the_agents() {
    let setup = setup();
    on_start(&setup.core);
    let core = &setup.core;
    let mode = || crate::sync_hub::agent_config_for(core.host(), ViewId(2)).scroll_mode;
    assert_eq!(mode(), rb_sync::AgentScrollMode::Anchor);
    let state = sync_set(core, &json!({ "scrollMode": "wheel" })).expect("set");
    assert_eq!(state.features.scroll_mode, rb_config::ScrollMode::Wheel);
    assert_eq!(mode(), rb_sync::AgentScrollMode::Wheel);
    // The published state names it, so the Firefox views' agents follow (review of RB-090-33).
    let published = core.host().shell_state().sync.expect("published");
    assert_eq!(published.features.scroll_mode, rb_config::ScrollMode::Wheel);
    crate::scroll_sync::on_config_changed(core);
    assert_eq!(mode(), rb_sync::AgentScrollMode::Anchor);
}
