use super::*;

#[test]
fn only_a_key_down_becomes_a_key() {
    assert_eq!(KeyDown::from_event(false, 0, 0x41, 97, 97), None);
    let key = KeyDown::from_event(true, 6, 0x20, 32, 32).expect("a key-down");
    assert_eq!(
        key,
        KeyDown {
            flags: 6,
            windows_key_code: 0x20,
            character: Some(' '),
            unmodified_character: Some(' '),
        }
    );
}

#[test]
fn a_broken_code_or_character_is_no_key_code_or_character() {
    let key = KeyDown::from_event(true, 0, -5, 0xD800, 65).expect("a key-down");
    assert_eq!(key.windows_key_code, 0);
    assert_eq!(key.character, None, "a lone surrogate is no character");
    assert_eq!(key.unmodified_character, Some('A'));
}
