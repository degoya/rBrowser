use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use super::*;

fn host() -> MemoryHost {
    MemoryHost::new()
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
}

#[test]
fn a_view_shows_its_own_state_over_every_views() {
    let mut state = State::default();
    assert!(!state.effective(None));
    state.set(Target::All, true);
    state.set(Target::View(3), false);
    assert!(state.effective(None));
    assert!(state.effective(Some(2)));
    assert!(!state.effective(Some(3)));
    state.set(Target::All, false);
    assert!(state.views.is_empty());
    assert!(!state.effective(Some(3)));
}

#[test]
fn the_state_goes_through_the_project_and_back() {
    let mut host = host();
    let mut state = State::default();
    state.set(Target::All, true);
    state.save(&mut host).expect("saved");
    let bytes = host.out.get(STATE_FILE).cloned().expect("written");
    host.project.insert(STATE_PATH.to_owned(), bytes);
    assert_eq!(State::load(&mut host), state);
}

#[test]
fn a_file_it_cannot_read_starts_off_and_bad_view_ids_are_dropped() {
    let mut host = host().with_file(STATE_PATH, b"{ nope");
    assert_eq!(State::load(&mut host), State::default());
    assert_eq!(host.logs.len(), 1);
    let file = json!({ "version": 1, "all": true, "views": { "4": false, "x": true } });
    let mut host = self::host().with_file(STATE_PATH, file.to_string().as_bytes());
    let state = State::load(&mut host);
    assert!(state.all);
    assert_eq!(state.views.len(), 1);
    let old = json!({ "version": 99, "all": true });
    let mut host = self::host().with_file(STATE_PATH, old.to_string().as_bytes());
    assert!(!State::load(&mut host).all);
}

#[test]
fn the_target_is_a_view_id_or_every_view() {
    assert_eq!(Target::from_args(&json!({ "target": 7 })), Target::View(7));
    assert_eq!(Target::from_args(&json!({ "target": "all" })), Target::All);
    assert_eq!(Target::from_args(&json!({})), Target::All);
    assert_eq!(Target::View(7).view(), Some(7));
}
