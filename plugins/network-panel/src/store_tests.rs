use super::*;
use crate::fake_host::FakeHost;

#[test]
fn the_sessions_switches_hold_for_that_session_only_and_switched_off_rules_always() {
    let mut host = FakeHost::default();
    let mut state = read(&mut host, "s1");
    assert_eq!(
        state,
        State {
            session: "s1".to_owned(),
            ..State::default()
        }
    );
    state.mocks_enabled = Some(false);
    state.cache_disabled = Some(true);
    state.clear_site_data = 2;
    state.disabled.insert("api.json#user".to_owned());
    write(&mut host, &state).expect("written");
    assert_eq!(read(&mut host, "s1"), state);
    let next = read(&mut host, "s2");
    assert_eq!(next.mocks_enabled, None);
    assert_eq!(next.cache_disabled, None);
    assert_eq!(next.clear_site_data, 0);
    assert!(next.disabled.contains("api.json#user"));
}

#[test]
fn the_switches_fall_back_to_the_settings() {
    let settings = Settings {
        cache_disabled: true,
        ..Settings::default()
    };
    let state = State::default();
    assert!(state.mocks_enabled(&settings));
    assert!(state.cache_disabled(&settings));
    let state = State {
        mocks_enabled: Some(false),
        cache_disabled: Some(false),
        ..State::default()
    };
    assert!(!state.mocks_enabled(&settings));
    assert!(!state.cache_disabled(&settings));
}

#[test]
fn without_a_session_no_switch_is_read() {
    let mut host = FakeHost::default();
    host.file(STATE_PATH, r#"{"session":"","mocksEnabled":false}"#);
    assert_eq!(read(&mut host, "").mocks_enabled, None);
}

#[test]
fn breakpoints_are_kept_across_sessions_and_the_asks_and_switch_for_one() {
    let mut host = FakeHost::default();
    let mut state = read(&mut host, "s1");
    state
        .breakpoints
        .push(json!({ "id": "bp-1", "urlPattern": "*" }));
    state.breakpoints_enabled = Some(false);
    state
        .asks
        .push(json!({ "id": "a1", "kind": "abort", "held": "h1" }));
    state.last_error = Some("unknown_held".to_owned());
    assert_eq!(state.next_seq(), 1);
    write(&mut host, &state).expect("written");
    assert_eq!(read(&mut host, "s1"), state);
    let next = read(&mut host, "s2");
    assert_eq!(next.breakpoints, state.breakpoints);
    assert_eq!(next.seq, 1);
    assert!(next.breakpoints_enabled() && next.asks.is_empty() && next.last_error.is_none());
}
