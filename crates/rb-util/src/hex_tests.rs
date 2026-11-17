use super::hex;

#[test]
fn two_lower_case_digits_per_byte() {
    assert_eq!(hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    assert_eq!(hex(&[0x00, 0xab, 0xff]), "00abff");
    assert_eq!(hex(&[]), "");
}

#[test]
fn a_digest_gives_sixty_four_digits() {
    let text = hex(&[0x5a; 32]);
    assert_eq!(text.len(), 64);
    assert!(
        text.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
}
