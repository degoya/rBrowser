use super::{Choice, STATE_FILE, STATE_PATH, Settings, State, clear, load, save};
use crate::catalog::Simulation;
use crate::testing::host;

fn choice(kind: Simulation, severity: Option<f64>) -> Choice {
    Choice { kind, severity }
}

#[test]
fn the_settings_have_their_defaults() {
    let mut host = host();
    let settings = Settings::read(&mut host);
    assert_eq!(settings.default, Simulation::None);
    assert_eq!(settings.blur_px, 2.0);
    assert!(settings.prefer_native);
    assert!(!settings.persist);
}

#[test]
fn the_settings_are_read_and_a_value_out_of_range_is_left_out() {
    let mut host = host();
    host.settings
        .insert("default".to_owned(), "\"glaucoma\"".to_owned());
    host.settings.insert("blurPx".to_owned(), "6".to_owned());
    host.settings
        .insert("preferNative".to_owned(), "false".to_owned());
    host.settings
        .insert("persist".to_owned(), "true".to_owned());
    let settings = Settings::read(&mut host);
    assert_eq!(settings.default, Simulation::Glaucoma);
    assert_eq!(settings.blur_px, 6.0);
    assert!(!settings.prefer_native);
    assert!(settings.persist);

    host.settings.insert("blurPx".to_owned(), "500".to_owned());
    host.settings
        .insert("default".to_owned(), "\"x-ray\"".to_owned());
    let settings = Settings::read(&mut host);
    assert_eq!(settings.blur_px, 2.0);
    assert_eq!(settings.default, Simulation::None);
    assert_eq!(
        host.logs
            .iter()
            .filter(|(level, _)| level == "warn")
            .count(),
        2
    );
}

#[test]
fn a_view_without_a_choice_gets_the_default_setting() {
    let mut state = State::default();
    let settings = Settings {
        default: Simulation::Dimmed,
        ..Settings::default()
    };
    assert_eq!(
        state.effective(4, &settings),
        choice(Simulation::Dimmed, None)
    );
    // An explicit "none" wins over the default.
    state.views.insert(4, choice(Simulation::None, None));
    assert_eq!(
        state.effective(4, &settings),
        choice(Simulation::None, None)
    );
}

#[test]
fn the_state_round_trips_through_the_out_folder() {
    let mut host = host();
    assert_eq!(load(&mut host), State::default());
    let mut state = State::default();
    state
        .views
        .insert(2, choice(Simulation::Protanomaly, Some(0.25)));
    state
        .views
        .insert(7, choice(Simulation::Deuteranopia, None));
    save(&mut host, &state).expect("written");
    let bytes = host.out.get(STATE_FILE).cloned().expect("written");
    let text = String::from_utf8(bytes.clone()).expect("UTF-8");
    assert!(text.contains("\"protanomaly\""), "{text}");
    let mut host = host.with_file(STATE_PATH, &bytes);
    assert_eq!(load(&mut host), state);
}

#[test]
fn an_unusable_state_file_or_entry_is_left_out() {
    let mut host = host().with_file(STATE_PATH, b"{ not json");
    assert_eq!(load(&mut host), State::default());
    assert!(host.logs.iter().any(|(level, _)| level == "warn"));

    let file = br#"{"version": 1, "views": {"1": {"type": "dimmed", "severity": 3},
        "2": {"type": "glaucoma", "severity": 0.5}}}"#;
    let mut host = crate::testing::host().with_file(STATE_PATH, file);
    let state = load(&mut host);
    assert_eq!(state.views.len(), 1);
    assert_eq!(
        state.views.get(&2),
        Some(&choice(Simulation::Glaucoma, Some(0.5)))
    );

    let other_version = br#"{"version": 9, "views": {}}"#;
    let mut host = crate::testing::host().with_file(STATE_PATH, other_version);
    assert_eq!(load(&mut host), State::default());
}

#[test]
fn clearing_empties_a_state_with_choices_and_writes_nothing_else() {
    let mut first = host();
    let mut state = State::default();
    state.views.insert(1, choice(Simulation::Sunlight, None));
    save(&mut first, &state).expect("written");

    let mut host = crate::testing::with_saved(host(), &first);
    clear(&mut host);
    let bytes = host.out.get(STATE_FILE).cloned().expect("written");
    let mut host = host.with_file(STATE_PATH, &bytes);
    assert_eq!(load(&mut host), State::default());

    let mut empty = crate::testing::host();
    clear(&mut empty);
    assert!(empty.out.is_empty());
}
