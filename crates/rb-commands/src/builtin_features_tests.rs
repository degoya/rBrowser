use super::*;
use crate::{CommandRegistry, Handler};

fn find(defs: &[CommandDef], id: &str) -> CommandDef {
    defs.iter()
        .find(|def| def.id.as_str() == id)
        .cloned()
        .unwrap_or_else(|| panic!("{id} is defined"))
}

#[test]
fn every_feature_command_registers_beside_the_builtin_ones() {
    let mut registry = CommandRegistry::new();
    let handler: Handler = std::sync::Arc::new(|_args| Ok(serde_json::Value::Null));
    for def in crate::builtin::definitions() {
        let id = def.id.to_string();
        registry.register(def, handler.clone()).expect(&id);
    }
    assert_eq!(definitions().len(), ROWS.len() + 9 * NUMBERED.len());
}

#[test]
fn the_numbered_families_follow_config_order() {
    let defs = definitions();
    let preset = find(&defs, "layout.preset.2");
    assert_eq!(
        preset.default_bindings[0].to_string(),
        "Ctrl+Shift+Space L 2"
    );
    assert_eq!(preset.title_key, "commands.layout.preset.2");
    let environment = find(&defs, "environments.switch.9");
    assert_eq!(environment.group, Group::Environments);
    assert_eq!(
        environment.mcp,
        McpCoverage::Covered("environment_set".into())
    );
}

#[test]
fn no_trust_command_is_reachable_over_mcp() {
    let defs = definitions();
    for id in ["trust.show", "trust.grant", "trust.revoke"] {
        assert_eq!(
            find(&defs, id).mcp,
            McpCoverage::Omitted(TRUST_OMITTED.into()),
            "{id}"
        );
    }
}

#[test]
fn the_devtools_chords_are_the_job_files() {
    let defs = definitions();
    assert_eq!(
        find(&defs, "devtools.toggle").default_bindings[0].to_string(),
        "Ctrl+Shift+Space D D"
    );
    assert_eq!(
        find(&defs, "devtools.dock_cycle").default_bindings[0].to_string(),
        "Ctrl+Shift+Space D W"
    );
}

#[test]
fn the_sync_chords_are_the_job_files() {
    let defs = definitions();
    for (id, chord) in [
        ("sync.skip_next", "Ctrl+Shift+Space S N"),
        ("sync.resync", "Ctrl+Shift+Space S R"),
        ("sync.toggle_view", "Ctrl+Shift+Space S X"),
        ("sync.toggle_view_navigation", "Ctrl+Shift+Space S U"),
        ("sync.toggle_scroll", "Ctrl+Shift+Space S C"),
        ("sync.toggle_focus", "Ctrl+Shift+Space S F"),
    ] {
        let def = find(&defs, id);
        assert_eq!(def.default_bindings[0].to_string(), chord, "{id}");
        assert_eq!(def.group, Group::Sync, "{id}");
        assert_eq!(def.mcp, McpCoverage::Covered("sync_set".into()), "{id}");
    }
}

#[test]
fn the_external_window_chords_are_free_and_named() {
    let defs = definitions();
    for (id, chord, group, mcp) in [
        (
            "view.external.arrange",
            "Ctrl+Shift+Space V D",
            Group::View,
            McpCoverage::Omitted(EXTERNAL_WINDOWS_OMITTED.into()),
        ),
        (
            "view.external.raise",
            "Ctrl+Shift+Space V W",
            Group::View,
            McpCoverage::Omitted(EXTERNAL_WINDOWS_OMITTED.into()),
        ),
        (
            "engine.restart",
            "Ctrl+Shift+Space R E",
            Group::Reload,
            McpCoverage::Covered("engine_restart".into()),
        ),
    ] {
        let def = find(&defs, id);
        assert_eq!(def.default_bindings[0].to_string(), chord, "{id}");
        assert_eq!(def.group, group, "{id}");
        assert_eq!(def.mcp, mcp, "{id}");
    }
}

#[test]
fn the_firefox_toggle_is_r_f_and_covered_by_engine_restart() {
    let defs = definitions();
    let def = find(&defs, "engine.firefox.toggle");
    assert_eq!(def.default_bindings[0].to_string(), "Ctrl+Shift+Space R F");
    assert_eq!(def.group, Group::Reload);
    assert_eq!(def.mcp, McpCoverage::Covered("engine_restart".into()));
}

#[test]
fn the_webkit_toggle_is_leader_v_k_and_covered_by_engine_restart() {
    let defs = definitions();
    let def = find(&defs, "engine.webkit.toggle");
    assert_eq!(def.default_bindings[0].to_string(), "Ctrl+Shift+Space V K");
    assert_eq!(def.group, Group::View);
    assert_eq!(def.mcp, McpCoverage::Covered("engine_restart".into()));
    assert_eq!(def.title_key, "commands.engine.webkit.toggle");
}

#[test]
fn the_recorder_chords_are_in_the_flows_group() {
    let defs = definitions();
    for (id, chord, tool) in [
        ("recorder.start", "Ctrl+Shift+Space F S", "recorder_start"),
        ("recorder.stop", "Ctrl+Shift+Space F T", "recorder_stop"),
        ("recorder.pause", "Ctrl+Shift+Space F P", "recorder_stop"),
        (
            "recorder.export_flow",
            "Ctrl+Shift+Space F W",
            "recorder_export",
        ),
        (
            "recorder.export_spec",
            "Ctrl+Shift+Space F E",
            "recorder_export",
        ),
    ] {
        let def = find(&defs, id);
        assert_eq!(def.default_bindings[0].to_string(), chord, "{id}");
        assert_eq!(def.group, Group::Flows, "{id}");
        assert_eq!(def.mcp, McpCoverage::Covered(tool.into()), "{id}");
    }
}

#[test]
fn visual_regression_has_its_chords_and_no_git_change_over_mcp() {
    let defs = definitions();
    let compare = find(&defs, "visual.compare");
    assert_eq!(
        compare.default_bindings[0].to_string(),
        "Ctrl+Shift+Space C R"
    );
    assert_eq!(compare.mcp, McpCoverage::Covered("visual_compare".into()));
    let update = find(&defs, "visual.baseline_update");
    assert_eq!(
        update.default_bindings[0].to_string(),
        "Ctrl+Shift+Space C B"
    );
    assert_eq!(
        update.mcp,
        McpCoverage::Covered("visual_baseline_update".into())
    );
    let lfs = find(&defs, "visual.lfs_setup");
    assert_eq!(lfs.default_bindings[0].to_string(), "Ctrl+Shift+Space C G");
    assert_eq!(lfs.mcp, McpCoverage::Omitted(LFS_SETUP_OMITTED.into()));
}

#[test]
fn the_throttle_toggle_is_leader_v_q_and_covered_by_settings_set() {
    // RB-020-09: the job's `V P` is the command palette's; `Q` for "quiet the hidden views".
    let defs = definitions();
    let def = find(&defs, "performance.toggle_throttle");
    assert_eq!(def.default_bindings[0].to_string(), "Ctrl+Shift+Space V Q");
    assert_eq!(def.group, Group::View);
    assert_eq!(def.mcp, McpCoverage::Covered("settings_set".into()));
    assert_eq!(def.title_key, "commands.performance.toggle_throttle");
}
