use super::*;

#[test]
fn a_token_is_64_hex_digits_and_new_each_time() {
    let one = Token::generate().expect("random");
    let two = Token::generate().expect("random");
    assert_eq!(one.expose().len(), 64);
    assert!(one.expose().bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(one, two);
}

#[test]
fn only_the_same_text_matches() {
    let token = Token::generate().expect("random");
    let text = token.expose().to_owned();
    assert!(token.matches(&text));
    assert!(!token.matches(&text[..63]));
    assert!(!token.matches(""));
    let mut other = text.into_bytes();
    other[10] = if other[10] == b'0' { b'1' } else { b'0' };
    assert!(!token.matches(&String::from_utf8(other).expect("ascii")));
}

#[test]
fn the_token_never_shows_in_debug_output() {
    let token = Token::generate().expect("random");
    let printed = format!("{token:?}");
    assert!(!printed.contains(token.expose()));
    assert_eq!(printed, "Token(<redacted>)");
}
