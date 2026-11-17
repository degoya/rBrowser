use std::sync::Arc;

use rb_commands::{CommandDef, CommandId, Group, Handler, McpCoverage};
use serde_json::Value;

use super::*;

fn def(id: &str, title_key: &str) -> CommandDef {
    CommandDef {
        id: CommandId::new(id).expect(id),
        title_key: title_key.to_owned(),
        group: Group::Sync,
        default_bindings: vec!["Ctrl+Shift+Space S T".parse().expect("a binding")],
        mcp: McpCoverage::Omitted("test".into()),
    }
}

fn noop() -> Handler {
    Arc::new(|_args| Ok(Value::Null))
}

/// RB-080-58: the status bar said `plugin.lighthouse.run failed: ...` - it names the command by
/// its title.
#[test]
fn a_failed_command_is_named_by_its_title() {
    let locale = br#"{"plugin":{"title-test":{"commands":{"run":"Run the title test"}}}}"#;
    rb_i18n::register_plugin_locale("title-test", "en", locale).expect("registers");
    let mut registry = CommandRegistry::new();
    registry
        .register(def("sync.toggle", "commands.sync.toggle"), noop())
        .expect("registers");
    registry
        .register_plugin(
            // A plugin's id `title-test` is `title_test` in its command ids.
            def("plugin.title_test.run", "plugin.title-test.commands.run"),
            noop(),
        )
        .expect("registers");
    assert_eq!(command_title(&registry, "sync.toggle"), "Sync on/off");
    assert_eq!(
        command_title(&registry, "plugin.title_test.run"),
        "Run the title test"
    );
    // Unknown, or no text: the id stands in.
    assert_eq!(command_title(&registry, "nope.nothing"), "nope.nothing");
}

/// A plugin command's title comes from its own plugin's catalogue only.
#[test]
fn a_plugin_command_cannot_take_another_plugins_title() {
    let locale = br#"{"plugin":{"title-victim":{"commands":{"run":"Victim title"}}}}"#;
    rb_i18n::register_plugin_locale("title-victim", "en", locale).expect("registers");
    let mut registry = CommandRegistry::new();
    registry
        .register_plugin(
            def("plugin.title_thief.run", "plugin.title-victim.commands.run"),
            noop(),
        )
        .expect("registers");
    assert_eq!(
        command_title(&registry, "plugin.title_thief.run"),
        "plugin.title_thief.run"
    );
}

/// RB-080-82: the shell gets a plugin command's title message (unformatted) from the command's
/// own plugin only; a core command has none.
#[test]
fn the_title_message_of_a_plugin_command_for_the_shell() {
    let locale = br#"{"plugin":{"title-shell":{"commands":{"add":"Title shell: Add {what}"}}}}"#;
    rb_i18n::register_plugin_locale("title-shell", "en", locale).expect("registers");
    assert_eq!(
        plugin_title_message("plugin.title_shell.add", "plugin.title-shell.commands.add")
            .as_deref(),
        Some("Title shell: Add {what}")
    );
    // Another plugin's key, a core command, an unknown text: none.
    assert_eq!(
        plugin_title_message("plugin.thief.add", "plugin.title-shell.commands.add"),
        None
    );
    assert_eq!(
        plugin_title_message("sync.toggle", "commands.sync.toggle"),
        None
    );
    assert_eq!(
        plugin_title_message(
            "plugin.title_shell.nope",
            "plugin.title-shell.commands.nope"
        ),
        None
    );
}
