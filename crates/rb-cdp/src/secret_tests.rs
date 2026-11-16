use super::*;

#[test]
fn random_hex_is_twice_as_many_lower_case_digits_and_new_each_time() {
    let one = random_hex(32).expect("random");
    let two = random_hex(32).expect("random");
    assert_eq!(one.len(), 64);
    assert!(
        one.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_ne!(one, two);
    assert_eq!(random_hex(0).expect("random"), "");
}

#[test]
fn hex_is_lower_case_and_keeps_leading_zeros() {
    assert_eq!(hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    assert_eq!(hex(&[]), "");
}

#[test]
fn constant_time_eq_compares_whole_slices() {
    assert!(constant_time_eq(b"abc", b"abc"));
    assert!(!constant_time_eq(b"abc", b"abd"));
    assert!(!constant_time_eq(b"abc", b"ab"));
    assert!(constant_time_eq(b"", b""));
}
