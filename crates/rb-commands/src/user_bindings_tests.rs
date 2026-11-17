use std::collections::BTreeMap;

use super::*;
use crate::Group;
use crate::registry::tests::{def, noop};

fn id(text: &str) -> CommandId {
    CommandId::new(text).expect(text)
}

fn binding(text: &str) -> Binding {
    text.parse().expect(text)
}

fn registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    for (command, chord) in [
        ("sync.toggle", "Ctrl+Shift+Space S S"),
        ("sync.skip", "Ctrl+Shift+Space S N"),
    ] {
        registry
            .register(def(command, Group::Sync, &[chord]), noop())
            .expect("registers");
    }
    registry
}

fn map(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    entries
        .iter()
        .map(|(id, texts)| {
            (
                (*id).to_owned(),
                texts.iter().map(|t| (*t).to_owned()).collect(),
            )
        })
        .collect()
}

#[test]
fn a_recorded_binding_works_at_once() {
    let mut registry = registry();
    let toggle = id("sync.toggle");
    registry
        .set_bindings(&toggle, vec![binding("Ctrl+Shift+7")])
        .expect("free");
    assert!(registry.is_changed(&toggle));
    assert_eq!(registry.resolve(&binding("Ctrl+Shift+7")), Some(&toggle));
    assert_eq!(
        registry.resolve(&binding("Ctrl+Shift+Space S S")),
        None,
        "the user's bindings replace the defaults"
    );
    registry.reset(&toggle);
    assert!(!registry.is_changed(&toggle));
    assert_eq!(
        registry.resolve(&binding("Ctrl+Shift+Space S S")),
        Some(&toggle)
    );
}

#[test]
fn reserved_and_taken_bindings_are_refused_in_the_editor() {
    let mut registry = registry();
    let toggle = id("sync.toggle");
    assert_eq!(
        registry.set_bindings(&toggle, vec![binding("Ctrl+T")]),
        Err(CommandError::Reserved(
            "shortcuts.reserved.browser_new_tab".into()
        ))
    );
    assert_eq!(
        registry.set_bindings(&toggle, vec![binding("Ctrl+Shift+Space S N")]),
        Err(CommandError::Conflict {
            binding: "Ctrl+Shift+Space S N".into(),
            other: "sync.skip".into()
        })
    );
    assert_eq!(
        registry.set_bindings(&id("sync.none"), vec![]),
        Err(CommandError::UnknownCommand("sync.none".into()))
    );
    assert!(!registry.is_changed(&toggle));
}

#[test]
fn removing_every_binding_is_a_change_and_setting_the_defaults_is_none() {
    let mut registry = registry();
    let toggle = id("sync.toggle");
    registry.set_bindings(&toggle, vec![]).expect("unbinds");
    assert!(registry.is_changed(&toggle));
    assert!(registry.bindings(&toggle).is_empty());
    registry
        .set_bindings(&toggle, vec![binding("Ctrl+Shift+Space S S")])
        .expect("the defaults");
    assert!(!registry.is_changed(&toggle));
    assert!(registry.user_bindings().is_empty());
}

#[test]
fn export_then_import_gives_the_same_bindings() {
    let mut registry = registry();
    registry
        .set_bindings(&id("sync.toggle"), vec![binding("Ctrl+Shift+7")])
        .expect("free");
    registry
        .set_bindings(&id("sync.skip"), vec![])
        .expect("unbinds");
    let exported = registry.export_json();
    let before = registry.user_bindings();

    let mut fresh = self::registry();
    let notices = fresh.import_json(&exported).expect("imports");
    assert!(notices.is_empty(), "{notices:?}");
    assert_eq!(fresh.user_bindings(), before);
    assert_eq!(
        fresh.resolve(&binding("Ctrl+Shift+7")),
        Some(&id("sync.toggle"))
    );
}

#[test]
fn two_commands_may_swap_keys_when_loaded() {
    let mut registry = registry();
    let notices = registry.load_user_bindings(&map(&[
        ("sync.skip", &["Ctrl+Shift+Space S S"]),
        ("sync.toggle", &["Ctrl+Shift+Space S N"]),
    ]));
    assert!(notices.is_empty(), "{notices:?}");
    assert_eq!(
        registry.resolve(&binding("Ctrl+Shift+Space S S")),
        Some(&id("sync.skip"))
    );
}

#[test]
fn what_does_not_fit_is_skipped_with_a_notice() {
    let mut registry = registry();
    let notices = registry.load_user_bindings(&map(&[
        ("sync.gone", &["Ctrl+Shift+5"]),
        ("sync.toggle", &["Ctrl+W"]),
        ("sync.skip", &["Ctrl+Shift+Space S S"]),
    ]));
    let keys: Vec<(&str, &str)> = notices
        .iter()
        .map(|n| (n.command.as_str(), n.message_key.as_str()))
        .collect();
    assert_eq!(
        keys,
        [
            ("sync.gone", "shortcuts.notice.unknown_command"),
            ("sync.toggle", "shortcuts.notice.user_binding_refused"),
            ("sync.skip", "shortcuts.notice.user_binding_refused"),
        ]
    );
    assert_eq!(
        notices[1].reason_key.as_deref(),
        Some("shortcuts.reserved.browser_close_tab")
    );
    assert_eq!(
        notices[2].reason_key.as_deref(),
        Some("shortcuts.rejected.conflict")
    );
    assert!(
        registry.user_bindings().is_empty(),
        "both keep their defaults"
    );
}

#[test]
fn loading_returns_its_notices_and_keeps_no_copy() {
    let mut registry = registry();
    let entries = map(&[("sync.gone", &["Ctrl+Shift+5"])]);
    assert_eq!(registry.load_user_bindings(&entries).len(), 1);
    assert_eq!(registry.load_user_bindings(&entries).len(), 1);
    assert!(
        registry.notices().is_empty(),
        "the notices go to the caller only, so none is shown twice or late"
    );
}

#[test]
fn import_refuses_a_foreign_file() {
    let mut registry = registry();
    assert!(registry.import_json("not json").is_err());
    assert!(
        registry
            .import_json(r#"{ "version": 9, "bindings": {} }"#)
            .is_err()
    );
}

/// A snapshot puts the user's bindings back as they were (a batch that failed halfway).
#[test]
fn a_snapshot_restores_the_user_bindings() {
    let mut registry = registry();
    let before = registry.user_snapshot();
    registry
        .set_bindings(&id("sync.toggle"), vec![binding("Ctrl+Shift+Space S Q")])
        .expect("free");
    assert!(registry.is_changed(&id("sync.toggle")));
    registry.restore_user(before.clone());
    assert!(!registry.is_changed(&id("sync.toggle")));
    assert_eq!(registry.user_snapshot(), before);
}
