use super::*;
use crate::builtin_reserved;

fn binding(text: &str) -> Binding {
    text.parse().expect(text)
}

fn check(text: &str) -> Result<(), CommandError> {
    check_binding(&binding(text), builtin_reserved(), Platform::Windows)
}

#[test]
fn free_bindings_pass() {
    for text in [
        "Ctrl+Shift+2",
        "F4",
        "Ctrl+Shift+Space S S",
        "Ctrl+Shift+Space ]",
        "Ctrl+Shift+Space .",
        "Ctrl+Shift+Space B 1",
    ] {
        assert_eq!(check(text), Ok(()), "{text}");
    }
}

#[test]
fn reserved_direct_bindings_are_refused_with_their_reason() {
    assert_eq!(
        check("Ctrl+T"),
        Err(CommandError::Reserved(
            "shortcuts.reserved.browser_new_tab".into()
        ))
    );
    assert_eq!(
        check("Ctrl+Alt+Q"),
        Err(CommandError::Reserved("shortcuts.reserved.altgr_at".into()))
    );
    assert_eq!(
        check("Ctrl+Shift+Space"),
        Err(CommandError::Reserved(REJECTED_LEADER.into()))
    );
}

#[test]
fn malformed_chords_are_refused() {
    assert_eq!(
        check("Ctrl+Shift+Space Escape"),
        Err(CommandError::InvalidBinding(
            "Ctrl+Shift+Space Escape".into()
        ))
    );
    assert_eq!(
        check("Ctrl+Shift+Space Z Z"),
        Err(CommandError::InvalidBinding("Ctrl+Shift+Space Z Z".into())),
        "a two-key chord starts with a group key"
    );
    assert_eq!(
        check("Ctrl+Shift+Space S"),
        Err(CommandError::Conflict {
            binding: "Ctrl+Shift+Space S".into(),
            other: "commands.group.sync".into()
        }),
        "a single key that opens a group"
    );
    assert!(check("Ctrl+Shift+Space ,").is_err(), "`,` opens Settings");
}

#[test]
fn conflicts_are_equality_or_a_chord_prefix() {
    assert!(bindings_conflict(
        &binding("Ctrl+Shift+1"),
        &binding("Ctrl+Shift+1")
    ));
    assert!(!bindings_conflict(
        &binding("Ctrl+Shift+1"),
        &binding("Ctrl+Shift+2")
    ));
    assert!(bindings_conflict(
        &binding("Ctrl+Shift+Space S S"),
        &binding("Ctrl+Shift+Space S S")
    ));
    assert!(bindings_conflict(
        &binding("Ctrl+Shift+Space ]"),
        &binding("Ctrl+Shift+Space ] 1")
    ));
    assert!(!bindings_conflict(
        &binding("Ctrl+Shift+Space S S"),
        &binding("Ctrl+Shift+Space S N")
    ));
}

#[test]
fn rejections_map_to_i18n_keys() {
    assert_eq!(
        rejection_key(&CommandError::Reserved(
            "shortcuts.reserved.altgr_at".into()
        )),
        "shortcuts.reserved.altgr_at"
    );
    assert_eq!(
        rejection_key(&CommandError::Conflict {
            binding: "F4".into(),
            other: "view.zoom".into()
        }),
        REJECTED_CONFLICT
    );
    assert_eq!(
        rejection_key(&CommandError::InvalidBinding("x".into())),
        REJECTED_INVALID
    );
}

#[test]
fn group_keys_are_found() {
    assert_eq!(group_of_key(Key::Char('S')), Some(Group::Sync));
    assert_eq!(group_of_key(Key::Char(',')), Some(Group::Settings));
    assert_eq!(group_of_key(Key::Char(']')), None);
}
