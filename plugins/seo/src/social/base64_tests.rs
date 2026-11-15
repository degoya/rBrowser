use super::*;

#[test]
fn encodes_the_rfc_4648_vectors() {
    for (plain, encoded) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(encode(plain.as_bytes()), encoded, "{plain}");
    }
    assert_eq!(encode(&[0xFF, 0xFE, 0xFD]), "//79");
}
