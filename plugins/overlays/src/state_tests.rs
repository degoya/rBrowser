use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::*;

fn on() -> OverlayState {
    OverlayState {
        enabled: true,
        options: Map::new(),
    }
}

#[test]
fn the_target_is_a_view_id_or_every_view() {
    assert_eq!(Target::from_args(&json!({ "target": 3 })), Target::View(3));
    assert_eq!(Target::from_args(&json!({ "target": "all" })), Target::All);
    assert_eq!(Target::from_args(&json!({})), Target::All);
    assert_eq!(Target::from_args(&json!({ "target": -1 })), Target::All);
}

#[test]
fn a_view_of_its_own_wins_over_every_view() {
    let mut state = State::default();
    state.set(Target::All, "overflow", on());
    state.set(Target::View(2), "overflow", OverlayState::default());
    assert!(state.effective(None, "overflow").enabled);
    assert!(state.effective(Some(1), "overflow").enabled);
    assert!(!state.effective(Some(2), "overflow").enabled);
    assert!(!state.effective(Some(1), "outlines").enabled);
}

#[test]
fn setting_every_view_drops_what_single_views_had() {
    let mut state = State::default();
    state.set(Target::View(2), "outlines", on());
    state.set(Target::View(2), "overflow", on());
    state.set(Target::All, "outlines", OverlayState::default());
    assert!(!state.effective(Some(2), "outlines").enabled);
    assert!(state.effective(Some(2), "overflow").enabled);
    state.set(Target::All, "overflow", OverlayState::default());
    assert!(state.views.is_empty());
}

#[test]
fn off_switches_everything_off_and_keeps_the_options() {
    let mut state = State::default();
    let mut highlight = on();
    highlight
        .options
        .insert("selector".to_owned(), json!(".card"));
    state.set(Target::All, "highlight", highlight);
    state.set(Target::View(4), "outlines", on());
    state.off();
    assert!(state.views.is_empty());
    let after = state.effective(None, "highlight");
    assert!(!after.enabled);
    assert_eq!(after.options["selector"], json!(".card"));
}

#[test]
fn the_state_goes_to_the_out_folder_and_comes_back_from_the_project() {
    let mut host = MemoryHost::new()
        .grant(Capability::OutWrite)
        .grant(Capability::ProjectRead);
    let mut state = State::default();
    state.set(Target::All, "overflow", on());
    state.save(&mut host).expect("saved");
    let written = host.out.get(STATE_FILE).cloned().expect("written");
    host.project.insert(STATE_PATH.to_owned(), written);
    assert_eq!(State::load(&mut host), state);
}

#[test]
fn a_missing_or_broken_state_is_every_overlay_off() {
    let mut host = MemoryHost::new().grant(Capability::ProjectRead);
    assert_eq!(State::load(&mut host), State::default());
    host.project
        .insert(STATE_PATH.to_owned(), b"{not json".to_vec());
    assert_eq!(State::load(&mut host), State::default());
    assert_eq!(host.logs.len(), 1);
    host.project.insert(
        STATE_PATH.to_owned(),
        br#"{"version":99,"all":{"overflow":{"enabled":true}}}"#.to_vec(),
    );
    assert_eq!(State::load(&mut host), State::default());
}

#[test]
fn without_the_capabilities_the_state_is_the_default_and_saving_fails() {
    let mut host = MemoryHost::new();
    assert_eq!(State::load(&mut host), State::default());
    assert!(State::default().save(&mut host).is_err());
}

#[test]
fn a_loaded_state_keeps_only_known_overlays_with_valid_options() {
    let mut host = MemoryHost::new().grant(Capability::ProjectRead);
    let long = "a".repeat(crate::overlay::MAX_SELECTOR_CHARS + 1);
    let file = json!({
        "version": 1,
        "all": {
            "overflow": { "enabled": true },
            "zindex": { "enabled": true },
            "highlight": { "enabled": true, "options": { "selector": long } }
        },
        "views": { "2": { "outlines": { "enabled": true, "options": { "depth": 3 } } } }
    });
    host.project
        .insert(STATE_PATH.to_owned(), file.to_string().into_bytes());
    let state = State::load(&mut host);
    assert_eq!(state.all.keys().collect::<Vec<_>>(), ["overflow"]);
    assert!(state.views.is_empty());
}
