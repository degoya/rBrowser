use std::fs;
use std::path::Path;
use std::sync::Arc;

use rb_commands::CommandRegistry;
use serde_json::Value;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup, register_all};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

/// Every command of the app, registered as the window does.
fn registry() -> CommandRegistry {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui, Handle::current(), CoreSetup::default());
    let mut registry = CommandRegistry::new();
    let refused = register_all(&mut registry, &core, None);
    assert!(refused.is_empty(), "{refused:?}");
    registry
}

#[tokio::test]
async fn the_document_lists_every_command_by_group_with_its_defaults_and_mcp() {
    let registry = registry();
    let document: Value = serde_json::from_str(&commands_document(&registry)).expect("JSON");
    assert_eq!(document["version"], 1);
    assert_eq!(document["leader"], "Ctrl+Shift+Space");
    let groups = document["groups"].as_array().expect("groups");
    assert_eq!(groups.len(), 14);
    assert_eq!(groups[0]["id"], "breakpoints");
    assert_eq!(groups[0]["key"], "B");
    assert_eq!(groups[0]["titleKey"], "commands.group.breakpoints");
    let commands = document["commands"].as_array().expect("commands");
    assert_eq!(commands.len(), registry.summaries().len());
    let back = commands
        .iter()
        .find(|command| command["id"] == "view.back")
        .expect("view.back");
    assert_eq!(back["group"], "view");
    assert!(back["titleKey"].as_str().is_some_and(|key| !key.is_empty()));
    assert!(
        !back["defaultBindings"]
            .as_array()
            .expect("bindings")
            .is_empty()
    );
    let diagnostics = commands
        .iter()
        .find(|command| command["id"] == "diagnostics.create")
        .expect("diagnostics.create");
    assert_eq!(diagnostics["mcp"]["covered"], "diagnostics");
    // In the groups' order, by id inside a group.
    let order: Vec<(usize, &str)> = commands
        .iter()
        .map(|command| {
            let group = groups
                .iter()
                .position(|group| group["id"] == command["group"])
                .expect("a known group");
            (group, command["id"].as_str().expect("id"))
        })
        .collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted);
}

/// `schema/commands.v1.json` is generated from the registry; this test fails when it is stale.
/// Regenerate with `RB_UPDATE_SCHEMA=1 cargo nextest run -p rbrowser commands_file`.
#[tokio::test]
async fn the_checked_in_commands_file_is_current() {
    let text = commands_document(&registry());
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(COMMANDS_FILE);
    if std::env::var_os("RB_UPDATE_SCHEMA").is_some_and(|value| value == "1") {
        fs::write(&path, &text).expect("write the commands file");
    }
    assert!(
        fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()),
        "{COMMANDS_FILE} is stale; regenerate with \
         RB_UPDATE_SCHEMA=1 cargo nextest run -p rbrowser commands_file"
    );
}

/// Every default chord of the registry as the window builds it, typed into the leader machine,
/// runs its command (RB-080-18; the bundled plugins' chords are walked in rb-commands' tests).
#[tokio::test]
async fn every_default_chord_of_the_window_reaches_its_command() {
    use rb_commands::{Binding, ChordMachine, Focus, KeyOutcome, Modifiers, Shortcut};
    use std::time::Instant;
    let registry = registry();
    let now = Instant::now();
    let mut wrong = Vec::new();
    let mut walked = 0;
    for def in registry.iter() {
        for binding in registry.default_bindings(&def.id) {
            let mut machine = ChordMachine::default();
            let outcome = match &binding {
                Binding::Direct(shortcut) => machine.on_key(&registry, Focus::View, *shortcut, now),
                Binding::Leader(chord) => {
                    let mut outcome = machine.on_key(&registry, Focus::View, LEADER, now);
                    for key in &chord.keys {
                        let shortcut = Shortcut {
                            modifiers: Modifiers::NONE,
                            key: *key,
                        };
                        outcome = machine.on_key(&registry, Focus::View, shortcut, now);
                    }
                    outcome
                }
            };
            walked += 1;
            if outcome != KeyOutcome::Run(def.id.clone()) {
                wrong.push(format!("{} {binding}: {outcome:?}", def.id));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "chords without their command:\n{}",
        wrong.join("\n")
    );
    assert!(walked > 50, "{walked} bindings walked");
    assert!(registry.iter().any(|def| def.id.as_str() == "debug.vitals"));
}
