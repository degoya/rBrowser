use std::collections::BTreeSet;

use rb_plugin_api::test_support::MemoryHost;
use serde_json::{Value, json};

use super::*;

fn host_with(settings: &[(&str, &str)]) -> MemoryHost {
    let mut host = MemoryHost::new();
    for (key, value) in settings {
        host.settings.insert((*key).to_owned(), (*value).to_owned());
    }
    host
}

#[test]
fn without_settings_the_defaults_hold() {
    let config = Config::read(&mut host_with(&[]));
    assert_eq!(config, Config::default());
    assert_eq!(
        config.to_json(),
        json!({ "enabled": true, "modifier": "Alt", "syncHighlight": true, "showBoxModel": true })
    );
}

#[test]
fn the_settings_are_read_by_property_name() {
    let config = Config::read(&mut host_with(&[
        ("enabled", "false"),
        ("modifier", "\"Shift\""),
        ("syncHighlight", "false"),
        ("showBoxModel", "false"),
    ]));
    assert!(!config.enabled);
    assert_eq!(config.modifier, "Shift");
    assert!(!config.sync_highlight);
    assert!(!config.show_box_model);
}

#[test]
fn a_modifier_outside_the_list_falls_back_to_alt() {
    for bad in [
        "\"Ctrl+Alt\"",
        "\"AltGraph\"",
        "\"Alt+Shift\"",
        "42",
        "not json",
    ] {
        let config = Config::read(&mut host_with(&[("modifier", bad)]));
        assert_eq!(config.modifier, "Alt", "{bad}");
    }
}

/// The reserved combinations (crates/rb-commands/data/reserved-shortcuts.json), as text.
fn reserved() -> Vec<(String, String)> {
    let text = include_str!("../../../crates/rb-commands/data/reserved-shortcuts.json");
    let data: Value = serde_json::from_str(text).expect("the reservation list is JSON");
    let entries = data["entries"]
        .as_array()
        .expect("a list of reserved combinations");
    entries
        .iter()
        .map(|entry| {
            (
                entry["combo"].as_str().expect("a combo").to_owned(),
                entry["source"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// Whether holding `modifier` alone (no other key) hits the reservation list: the list names
/// it bare, or it is the AltGr state (`Ctrl+Alt`), which on German, French and Spanish
/// layouts is typing, not a modifier.
fn held_alone_is_reserved(modifier: &str, reserved: &[(String, String)]) -> bool {
    let held: BTreeSet<&str> = modifier.split('+').collect();
    reserved.iter().any(|(combo, source)| {
        let keys: Vec<&str> = combo.split('+').collect();
        let (last, mods) = keys.split_last().expect("a combo has a key");
        let bare = keys.iter().copied().collect::<BTreeSet<_>>() == held;
        let any_with_exactly_these = *last == "<any>"
            && source == "altgr"
            && mods.iter().copied().collect::<BTreeSet<_>>() == held;
        bare || any_with_exactly_these
    })
}

#[test]
fn every_allowed_modifier_held_alone_passes_the_reservation_list() {
    let reserved = reserved();
    assert!(!reserved.is_empty());
    for modifier in MODIFIERS {
        assert!(
            !held_alone_is_reserved(modifier, &reserved),
            "{modifier} is reserved"
        );
    }
    // The check bites: Ctrl+Alt is AltGr.
    assert!(held_alone_is_reserved("Ctrl+Alt", &reserved));
}
