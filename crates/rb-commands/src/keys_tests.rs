use super::*;

#[test]
fn leader_is_ctrl_shift_space() {
    assert_eq!(LEADER.to_string(), "Ctrl+Shift+Space");
}

#[test]
fn shortcuts_display_in_canonical_order() {
    let shortcut = Shortcut {
        modifiers: Modifiers {
            ctrl: true,
            shift: true,
            alt: true,
            meta: true,
        },
        key: Key::Char('1'),
    };
    assert_eq!(shortcut.to_string(), "Ctrl+Shift+Alt+Win+1");
    let f9 = Shortcut {
        modifiers: Modifiers::NONE,
        key: Key::F(9),
    };
    assert_eq!(f9.to_string(), "F9");
    let plus = Shortcut {
        modifiers: Modifiers::CTRL,
        key: Key::Char('+'),
    };
    assert_eq!(plus.to_string(), "Ctrl+Plus", "`+` is spelled out");
}

#[test]
fn chords_and_bindings_display_with_the_leader() {
    let chord = Chord {
        keys: vec![Key::Char('L'), Key::Char('1')],
    };
    assert_eq!(chord.to_string(), "Ctrl+Shift+Space L 1");
    let binding = Binding::Leader(chord);
    assert_eq!(binding.to_string(), "Ctrl+Shift+Space L 1");
    let direct = Binding::Direct(Shortcut {
        modifiers: Modifiers::CTRL_SHIFT,
        key: Key::Char('2'),
    });
    assert_eq!(direct.to_string(), "Ctrl+Shift+2");
}

fn sc(modifiers: Modifiers, key: Key) -> Shortcut {
    Shortcut { modifiers, key }
}

#[test]
fn shortcuts_parse_their_text_form() {
    assert_eq!(
        "Ctrl+Shift+2".parse::<Shortcut>(),
        Ok(sc(Modifiers::CTRL_SHIFT, Key::Char('2')))
    );
    assert_eq!("F9".parse::<Shortcut>(), Ok(sc(Modifiers::NONE, Key::F(9))));
    assert_eq!(
        "Ctrl+Plus".parse::<Shortcut>(),
        Ok(sc(Modifiers::CTRL, Key::Char('+')))
    );
    assert_eq!(
        "Ctrl++".parse::<Shortcut>(),
        Ok(sc(Modifiers::CTRL, Key::Char('+'))),
        "a trailing `+` is the plus key"
    );
    assert_eq!(
        "Alt+Left".parse::<Shortcut>(),
        Ok(sc(
            Modifiers {
                alt: true,
                ..Modifiers::NONE
            },
            Key::Left
        ))
    );
    assert_eq!(
        "Ctrl+,".parse::<Shortcut>(),
        Ok(sc(Modifiers::CTRL, Key::Char(',')))
    );
}

#[test]
fn parsing_is_lenient_in_case_order_and_aliases() {
    let expected = sc(
        Modifiers {
            ctrl: true,
            shift: true,
            alt: true,
            meta: true,
        },
        Key::Char('K'),
    );
    for text in [
        "Ctrl+Shift+Alt+Win+K",
        "win+alt+shift+ctrl+k",
        "Control+Shift+Alt+Meta+K",
        " Ctrl+Shift+Alt+Win+K ",
    ] {
        assert_eq!(text.parse::<Shortcut>(), Ok(expected), "{text}");
    }
    for (text, key) in [
        ("Esc", Key::Escape),
        ("Return", Key::Enter),
        ("Del", Key::Delete),
        ("PgUp", Key::PageUp),
        ("pagedown", Key::PageDown),
        ("ArrowUp", Key::Up),
        ("f24", Key::F(24)),
        ("space", Key::Space),
    ] {
        assert_eq!(
            text.parse::<Shortcut>(),
            Ok(sc(Modifiers::NONE, key)),
            "{text}"
        );
    }
}

#[test]
fn bad_shortcuts_are_refused() {
    for bad in [
        "",
        "Ctrl+",
        "Ctrl+Shift",
        "Ctrl+Ctrl+A",
        "Ctrl+AB",
        "Hyper+A",
        "F0",
        "F25",
        "Ctrl+Shift+Space L",
        "Ctrl+ A",
    ] {
        assert_eq!(
            bad.parse::<Shortcut>(),
            Err(CommandError::InvalidBinding(bad.to_owned())),
            "{bad:?}"
        );
    }
}

#[test]
fn display_and_parse_round_trip() {
    for text in [
        "Ctrl+Shift+1",
        "F2",
        "Ctrl+Plus",
        "Ctrl+Shift+Alt+Win+Delete",
        "Shift+F10",
        "Alt+Home",
        "Ctrl+]",
        "PageDown",
    ] {
        let parsed: Shortcut = text.parse().expect(text);
        assert_eq!(parsed.to_string(), text);
    }
    for text in [
        "Ctrl+Shift+Space L 1",
        "Ctrl+Shift+Space ]",
        "Ctrl+Shift+Space S S",
    ] {
        let parsed: Binding = text.parse().expect(text);
        assert_eq!(parsed.to_string(), text);
    }
}

#[test]
fn chords_parse_with_one_or_two_keys() {
    assert_eq!(
        "Ctrl+Shift+Space L 1".parse::<Chord>(),
        Ok(Chord {
            keys: vec![Key::Char('L'), Key::Char('1')]
        })
    );
    assert_eq!(
        "shift+ctrl+space  s   n".parse::<Chord>(),
        Ok(Chord {
            keys: vec![Key::Char('S'), Key::Char('N')]
        })
    );
    assert_eq!(
        "Leader K S".parse::<Chord>(),
        Ok(Chord {
            keys: vec![Key::Char('K'), Key::Char('S')]
        }),
        "plugin manifests write `Leader`"
    );
    assert_eq!(
        "Ctrl+Shift+Space +".parse::<Chord>(),
        Ok(Chord {
            keys: vec![Key::Char('+')]
        })
    );
    for bad in [
        "Ctrl+Shift+Space",
        "Ctrl+Space L",
        "Ctrl+Shift+Space L 1 2",
        "Ctrl+Shift+Space Ctrl+L",
        "L 1",
    ] {
        assert_eq!(
            bad.parse::<Chord>(),
            Err(CommandError::InvalidBinding(bad.to_owned())),
            "{bad:?}"
        );
    }
}

#[test]
fn bindings_parse_either_form() {
    assert_eq!(
        "Ctrl+Shift+2".parse::<Binding>(),
        Ok(Binding::Direct(sc(Modifiers::CTRL_SHIFT, Key::Char('2'))))
    );
    assert_eq!(
        "Ctrl+Shift+Space ,".parse::<Binding>(),
        Ok(Binding::Leader(Chord {
            keys: vec![Key::Char(',')]
        }))
    );
    assert_eq!(
        "Ctrl+Shift+Space".parse::<Binding>(),
        Ok(Binding::Direct(LEADER)),
        "the bare leader is a direct combination; registering it is refused elsewhere"
    );
    assert!("nonsense words".parse::<Binding>().is_err());
}

#[test]
fn bindings_serialize_as_text() {
    let binding: Binding = "Ctrl+Shift+Space S S".parse().expect("parses");
    let json = serde_json::to_value(&binding).expect("serializes");
    assert_eq!(json, "Ctrl+Shift+Space S S");
    let back: Binding = serde_json::from_value(json).expect("deserializes");
    assert_eq!(back, binding);
}
