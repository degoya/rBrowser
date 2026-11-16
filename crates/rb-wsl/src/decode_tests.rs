use super::*;

fn utf16le(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[test]
fn utf16le_without_and_with_its_mark() {
    assert_eq!(decode(&utf16le("Ubuntu\r\n")), "Ubuntu\r\n");
    let mut marked = vec![0xFF, 0xFE];
    marked.extend(utf16le("Debian\n"));
    assert_eq!(decode(&marked), "Debian\n");
}

#[test]
fn utf8_with_and_without_its_mark() {
    assert_eq!(decode(b"Ubuntu\n"), "Ubuntu\n");
    assert_eq!(decode(b"\xEF\xBB\xBFUbuntu\n"), "Ubuntu\n");
}

#[test]
fn nul_padding_and_a_trailing_odd_byte_are_dropped() {
    let mut bytes = utf16le("Ubuntu\0\0");
    bytes.push(0x41);
    assert_eq!(decode(&bytes), "Ubuntu");
}

#[test]
fn a_few_characters_past_ascii_stay_utf16() {
    assert_eq!(
        decode(&utf16le("Wird ausgef\u{fc}hrt")),
        "Wird ausgef\u{fc}hrt"
    );
}

#[test]
fn nothing_is_nothing() {
    assert_eq!(decode(&[]), "");
}
