use base64::Engine as _;

use super::*;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/extensions/crx"
);

fn key(name: &str) -> Vec<u8> {
    let text = std::fs::read_to_string(format!("{FIXTURES}/fixtures.json")).expect("fixtures.json");
    let json: serde_json::Value = serde_json::from_str(&text).expect("json");
    base64::engine::general_purpose::STANDARD
        .decode(json[name].as_str().expect("key"))
        .expect("base64")
}

#[test]
fn an_rsa_key_yields_its_pkcs1_bytes() {
    let der = key("developerKey");
    let parsed = parse(&der).expect("parses");
    assert_eq!(parsed.kind, KeyKind::Rsa);
    // RSAPublicKey is a SEQUENCE of the modulus and the exponent.
    assert_eq!(parsed.key[0], 0x30);
    assert!(parsed.key.len() > 256, "a 2048-bit modulus");
}

#[test]
fn a_p256_key_yields_its_uncompressed_point() {
    let der = key("publisherKey");
    let parsed = parse(&der).expect("parses");
    assert_eq!(parsed.kind, KeyKind::EcP256);
    assert_eq!(parsed.key.len(), 65);
    assert_eq!(parsed.key[0], 0x04);
}

#[test]
fn anything_else_is_no_key() {
    assert!(parse(b"").is_none());
    assert!(parse(b"\x30\x03\x02\x01\x05").is_none());
    let mut rsa = key("developerKey");
    // The algorithm's OID changed by one byte.
    let position = rsa
        .windows(9)
        .position(|w| w == [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01])
        .expect("oid");
    rsa[position + 8] = 0x02;
    assert!(parse(&rsa).is_none());
    let truncated = key("publisherKey");
    assert!(parse(&truncated[..truncated.len() - 1]).is_none());
}
