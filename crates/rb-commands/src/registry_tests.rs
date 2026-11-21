use std::sync::Arc;

use serde_json::{Value, json};

use super::*;

pub(crate) fn def(id: &str, group: Group, bindings: &[&str]) -> CommandDef {
    CommandDef {
        id: CommandId::new(id).expect(id),
        title_key: format!("commands.{id}"),
        group,
        default_bindings: bindings
            .iter()
            .map(|text| text.parse().expect(text))
            .collect(),
        mcp: McpCoverage::Omitted("test".into()),
    }
}

pub(crate) fn noop() -> Handler {
    Arc::new(|_args| Ok(Value::Null))
}

fn id(text: &str) -> CommandId {
    CommandId::new(text).expect(text)
}

fn binding(text: &str) -> Binding {
    text.parse().expect(text)
}

#[test]
fn register_resolve_and_dispatch() {
    let mut registry = CommandRegistry::new();
    let echo: Handler = Arc::new(|args| Ok(json!({ "echo": args })));
    registry
        .register(
            def(
                "sync.toggle",
                Group::Sync,
                &["Ctrl+Shift+Space S S", "Ctrl+Shift+8"],
            ),
            echo,
        )
        .expect("registers");
    assert_eq!(registry.iter().count(), 1);
    assert_eq!(registry.iter_group(Group::Sync).count(), 1);
    assert_eq!(
        registry.resolve(&binding("Ctrl+Shift+Space S S")),
        Some(&id("sync.toggle"))
    );
    assert_eq!(
        registry.resolve(&binding("Ctrl+Shift+8")),
        Some(&id("sync.toggle"))
    );
    assert_eq!(registry.resolve(&binding("Ctrl+Shift+9")), None);
    assert_eq!(
        registry.dispatch(&id("sync.toggle"), json!(1)),
        Ok(json!({ "echo": 1 }))
    );
    assert_eq!(
        registry.dispatch(&id("sync.other"), Value::Null),
        Err(CommandError::UnknownCommand("sync.other".into()))
    );
}

#[test]
fn a_command_without_a_leader_chord_is_refused() {
    let mut registry = CommandRegistry::new();
    assert_eq!(
        registry.register(def("view.zoom", Group::View, &[]), noop()),
        Err(CommandError::InvalidBinding(
            "view.zoom: no leader chord".into()
        ))
    );
    assert_eq!(
        registry.register(def("view.zoom", Group::View, &["Ctrl+Shift+5"]), noop()),
        Err(CommandError::InvalidBinding(
            "view.zoom: no leader chord".into()
        ))
    );
    assert_eq!(registry.iter().count(), 0);
}

#[test]
fn reserved_defaults_and_duplicates_are_refused() {
    let mut registry = CommandRegistry::new();
    assert_eq!(
        registry.register(
            def("view.tab", Group::View, &["Ctrl+Shift+Space V T", "Ctrl+T"]),
            noop()
        ),
        Err(CommandError::Reserved(
            "shortcuts.reserved.browser_new_tab".into()
        ))
    );
    registry
        .register(
            def("view.a", Group::View, &["Ctrl+Shift+Space V A"]),
            noop(),
        )
        .expect("registers");
    assert_eq!(
        registry.register(
            def("view.a", Group::View, &["Ctrl+Shift+Space V B"]),
            noop()
        ),
        Err(CommandError::Duplicate("view.a".into()))
    );
    assert_eq!(
        registry.register(
            def("view.b", Group::View, &["Ctrl+Shift+Space V A"]),
            noop()
        ),
        Err(CommandError::Conflict {
            binding: "Ctrl+Shift+Space V A".into(),
            other: "view.a".into()
        })
    );
}

#[test]
fn a_third_party_plugin_default_that_collides_stays_unbound() {
    let mut registry = CommandRegistry::new();
    let notices = registry
        .register_plugin(
            def("plugin.evil.close", Group::Plugins, &["Ctrl+W"]),
            noop(),
        )
        .expect("registers");
    let close = id("plugin.evil.close");
    assert_eq!(registry.resolve(&binding("Ctrl+W")), None, "no direct key");
    assert_eq!(
        registry.bindings(&close),
        vec![binding("Ctrl+Shift+Space X A")],
        "reachable through the leader"
    );
    assert_eq!(notices.len(), 2);
    assert_eq!(notices[0].message_key, NOTICE_BINDING_DROPPED);
    assert_eq!(notices[0].binding.as_deref(), Some("Ctrl+W"));
    assert_eq!(
        notices[0].reason_key.as_deref(),
        Some("shortcuts.reserved.browser_close_tab")
    );
    assert_eq!(notices[1].message_key, NOTICE_LEADER_ASSIGNED);
    assert_eq!(registry.notices().len(), 2);
    assert_eq!(registry.take_notices().len(), 2);
    assert!(registry.notices().is_empty());
}

#[test]
fn plugin_chords_skip_taken_keys_and_core_ids_are_refused() {
    let mut registry = CommandRegistry::new();
    registry
        .register(
            def("app.plugins", Group::Plugins, &["Ctrl+Shift+Space X A"]),
            noop(),
        )
        .expect("registers");
    registry
        .register_plugin(def("plugin.seo.check", Group::Checks, &[]), noop())
        .expect("registers");
    assert_eq!(
        registry.bindings(&id("plugin.seo.check")),
        vec![binding("Ctrl+Shift+Space X B")]
    );
    assert_eq!(
        registry.register_plugin(def("seo.check", Group::Checks, &[]), noop()),
        Err(CommandError::InvalidId("seo.check".into()))
    );
}

#[test]
fn plugin_defaults_that_fit_are_kept() {
    let mut registry = CommandRegistry::new();
    let notices = registry
        .register_plugin(
            def("plugin.seo.check", Group::Checks, &["Ctrl+Shift+Space K S"]),
            noop(),
        )
        .expect("registers");
    assert!(notices.is_empty());
    assert_eq!(
        registry.resolve(&binding("Leader K S")),
        Some(&id("plugin.seo.check"))
    );
}

#[test]
fn the_editor_asks_before_binding() {
    let mut registry = CommandRegistry::new();
    registry
        .register(
            def("sync.toggle", Group::Sync, &["Ctrl+Shift+Space S S"]),
            noop(),
        )
        .expect("registers");
    registry
        .register(
            def("sync.skip", Group::Sync, &["Ctrl+Shift+Space S N"]),
            noop(),
        )
        .expect("registers");
    let skip = id("sync.skip");
    assert_eq!(
        registry.check_for(&skip, &binding("Ctrl+Shift+Space S N")),
        Ok(())
    );
    assert_eq!(
        registry.check_for(&skip, &binding("Ctrl+Shift+Space S S")),
        Err(CommandError::Conflict {
            binding: "Ctrl+Shift+Space S S".into(),
            other: "sync.toggle".into()
        })
    );
    assert_eq!(
        registry.check_for(&skip, &binding("Ctrl+T")),
        Err(CommandError::Reserved(
            "shortcuts.reserved.browser_new_tab".into()
        ))
    );
    assert_eq!(
        registry.check_for(&skip, &binding("Ctrl+Alt+Q")),
        Err(CommandError::Reserved("shortcuts.reserved.altgr_at".into()))
    );
}

#[test]
fn summaries_carry_what_the_shell_needs() {
    let mut registry = CommandRegistry::new();
    registry
        .register(
            def("sync.toggle", Group::Sync, &["Ctrl+Shift+Space S S"]),
            noop(),
        )
        .expect("registers");
    let summaries = registry.summaries();
    assert_eq!(summaries.len(), 1);
    let json = serde_json::to_value(&summaries[0]).expect("serializes");
    assert_eq!(json["id"], "sync.toggle");
    assert_eq!(json["titleKey"], "commands.sync.toggle");
    assert_eq!(json["group"], "sync");
    assert_eq!(json["bindings"], json!(["Ctrl+Shift+Space S S"]));
    assert_eq!(json["defaultBindings"], json!(["Ctrl+Shift+Space S S"]));
    assert_eq!(json["changed"], false);
    assert_eq!(json["mcp"], json!({ "omitted": "test" }));
}

#[test]
fn the_handler_is_handed_out_to_run_without_the_registry() {
    let mut registry = CommandRegistry::new();
    registry
        .register(
            def("sync.toggle", Group::Sync, &["Ctrl+Shift+Space S S"]),
            Arc::new(|_| Ok(Value::from(7))),
        )
        .expect("registers");
    let handler = registry.handler(&id("sync.toggle")).expect("known");
    drop(registry);
    assert_eq!(handler(Value::Null).expect("runs"), Value::from(7));
    assert!(CommandRegistry::new().handler(&id("sync.toggle")).is_none());
}

#[test]
fn a_plugin_s_commands_leave_the_registry_with_their_bindings() {
    // RB-092-06: a plugin removed or replaced at run time takes its commands along.
    let mut registry = CommandRegistry::new();
    registry
        .register(
            def("sync.toggle", Group::Sync, &["Ctrl+Shift+Space S S"]),
            noop(),
        )
        .expect("registers");
    for command in ["plugin.seo.check", "plugin.seo.audit", "plugin.seolint.run"] {
        registry
            .register_plugin(def(command, Group::Checks, &[]), noop())
            .expect("registers");
    }
    let chord = registry.bindings(&id("plugin.seo.check"));
    assert_eq!(chord, vec![binding("Ctrl+Shift+Space X A")]);
    let removed = registry.unregister_plugin("seo");
    assert_eq!(
        removed,
        vec![id("plugin.seo.audit"), id("plugin.seo.check")]
    );
    assert!(registry.get(&id("plugin.seo.check")).is_none());
    assert!(registry.handler(&id("plugin.seo.audit")).is_none());
    // Another plugin whose id starts alike and the core's commands stay.
    assert!(registry.get(&id("plugin.seolint.run")).is_some());
    assert!(registry.get(&id("sync.toggle")).is_some());
    // Its chord is free again: the same plugin, loaded anew, gets it back.
    assert_eq!(registry.resolve(&binding("Ctrl+Shift+Space X A")), None);
    registry
        .register_plugin(def("plugin.seo.check", Group::Checks, &[]), noop())
        .expect("registers again");
    assert_eq!(registry.bindings(&id("plugin.seo.check")), chord);
    // Nothing of a core command, nothing for an id that is none.
    assert!(registry.unregister_plugin("sync").is_empty());
    assert!(registry.unregister_plugin("").is_empty());
    assert!(registry.get(&id("sync.toggle")).is_some());
}

#[test]
fn a_hyphenated_plugin_s_commands_leave_too() {
    // A plugin id's `-` is `_` in its command ids (`PluginManifest::command_id`): removing
    // `width-sweep` removes `plugin.width_sweep.*`, nothing of another plugin.
    assert_eq!(plugin_command_prefix("width-sweep"), "plugin.width_sweep.");
    let mut registry = CommandRegistry::new();
    for command in [
        "plugin.width_sweep.run",
        "plugin.width_sweep.stop",
        "plugin.width.run",
    ] {
        registry
            .register_plugin(def(command, Group::Checks, &[]), noop())
            .expect("registers");
    }
    assert_eq!(
        registry.unregister_plugin("width-sweep"),
        vec![id("plugin.width_sweep.run"), id("plugin.width_sweep.stop")]
    );
    assert!(registry.get(&id("plugin.width.run")).is_some());
    // Registered again, as a live update does, without a duplicate.
    registry
        .register_plugin(def("plugin.width_sweep.run", Group::Checks, &[]), noop())
        .expect("registers again");
}
