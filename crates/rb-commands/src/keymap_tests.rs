use super::*;

#[test]
fn flags_map_to_modifiers() {
    assert_eq!(modifiers_from_event_flags(0), Modifiers::NONE);
    assert_eq!(
        modifiers_from_event_flags(EVENTFLAG_CONTROL_DOWN | EVENTFLAG_SHIFT_DOWN),
        Modifiers::CTRL_SHIFT
    );
    let altgr = modifiers_from_event_flags(EVENTFLAG_ALTGR_DOWN);
    assert!(altgr.ctrl && altgr.alt && !altgr.shift && !altgr.meta);
    assert!(modifiers_from_event_flags(EVENTFLAG_COMMAND_DOWN).meta);
    // Caps Lock (bit 0) and mouse buttons do not count.
    assert_eq!(modifiers_from_event_flags(1 | 1 << 4), Modifiers::NONE);
}

#[test]
fn virtual_keys_map_to_keys() {
    assert_eq!(key_from_windows_key_code(0x41, None), Some(Key::Char('A')));
    assert_eq!(key_from_windows_key_code(0x32, None), Some(Key::Char('2')));
    assert_eq!(key_from_windows_key_code(0x62, None), Some(Key::Char('2')));
    assert_eq!(key_from_windows_key_code(0x20, None), Some(Key::Space));
    assert_eq!(key_from_windows_key_code(0x25, None), Some(Key::Left));
    assert_eq!(key_from_windows_key_code(0x70, None), Some(Key::F(1)));
    assert_eq!(key_from_windows_key_code(0x87, None), Some(Key::F(24)));
    assert_eq!(key_from_windows_key_code(0x10, None), None, "Shift alone");
    assert_eq!(key_from_windows_key_code(0x2C, None), None, "Print Screen");
}

#[test]
fn punctuation_follows_the_layout() {
    assert_eq!(key_from_windows_key_code(0xDD, None), Some(Key::Char(']')));
    assert_eq!(key_from_windows_key_code(0xBC, None), Some(Key::Char(',')));
    // German layout: VK_OEM_1 types u-umlaut, VK_OEM_PLUS types `+`.
    assert_eq!(
        key_from_windows_key_code(0xBA, Some('\u{fc}')),
        Some(Key::Char('\u{fc}'))
    );
    assert_eq!(
        key_from_windows_key_code(0xBB, Some('+')),
        Some(Key::Char('+'))
    );
}

#[test]
fn typed_characters_map_to_keys() {
    assert_eq!(key_from_character(']'), Some(Key::Char(']')));
    assert_eq!(key_from_character('s'), Some(Key::Char('S')));
    assert_eq!(key_from_character('\r'), None);
    assert_eq!(key_from_character(' '), None);
}

#[test]
fn a_whole_event_maps_to_a_shortcut() {
    assert_eq!(
        shortcut_from_cef(EVENTFLAG_CONTROL_DOWN | EVENTFLAG_SHIFT_DOWN, 0x20, None),
        Some(crate::LEADER)
    );
    let altgr_q = shortcut_from_cef(EVENTFLAG_ALTGR_DOWN, 0x51, Some('q')).expect("maps");
    assert_eq!(altgr_q.to_string(), "Ctrl+Alt+Q");
}

// RB-090-63: a native menu shows a direct shortcut beside its row; the key goes back to its
// virtual-key code, the way `key_from_windows_key_code` came from it.
#[test]
fn keys_map_back_to_virtual_keys() {
    for code in (0x30..=0x39).chain(0x41..=0x5A).chain(0x70..=0x87).chain([
        0x08, 0x09, 0x0D, 0x1B, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2D, 0x2E,
        0xBC, 0xBD, 0xBE, 0xDB, 0xDD,
    ]) {
        let key = key_from_windows_key_code(code, None).expect("a key");
        assert_eq!(windows_key_code(key), Some(code), "{key}");
    }
    assert_eq!(windows_key_code(Key::Char('+')), Some(0xBB));
    assert_eq!(windows_key_code(Key::Char('\u{fc}')), None);
    assert_eq!(windows_key_code(Key::F(25)), None);
}
