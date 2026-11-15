use super::*;

/// The fixtures of `tests/fixtures/extensions/crx/make_fixtures.py`.
const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/extensions/crx"
);

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).expect("the fixture exists")
}

struct Expected {
    id: String,
    publisher: [u8; 32],
}

fn expected() -> Expected {
    let text = std::fs::read_to_string(format!("{FIXTURES}/fixtures.json")).expect("fixtures.json");
    let json: serde_json::Value = serde_json::from_str(&text).expect("json");
    let hex = json["publisherKeyHash"].as_str().expect("hash");
    let mut publisher = [0_u8; 32];
    for (index, byte) in publisher.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).expect("hex");
    }
    Expected {
        id: json["id"].as_str().expect("id").to_owned(),
        publisher,
    }
}

#[test]
fn a_valid_crx_verifies_and_hands_out_its_zip() {
    let want = expected();
    let bytes = fixture("valid.crx");
    let verified = verify(&bytes, &want.id, &want.publisher).expect("verifies");
    assert_eq!(verified.id, want.id);
    assert!(verified.zip.starts_with(b"PK"), "the payload is the ZIP");
    assert_eq!(
        super::super::manifest::id_from_der(&verified.public_key),
        want.id
    );
}

#[test]
fn a_changed_byte_is_a_bad_signature() {
    let want = expected();
    let error =
        verify(&fixture("bad-signature.crx"), &want.id, &want.publisher).expect_err("refused");
    assert_eq!(error, Crx3Error::BadSignature);
}

#[test]
fn another_extensions_crx_is_an_id_mismatch() {
    let want = expected();
    let text = std::fs::read_to_string(format!("{FIXTURES}/fixtures.json")).expect("fixtures.json");
    let json: serde_json::Value = serde_json::from_str(&text).expect("json");
    let other = json["otherId"].as_str().expect("otherId").to_owned();
    let error = verify(&fixture("wrong-id.crx"), &want.id, &want.publisher).expect_err("refused");
    assert_eq!(
        error,
        Crx3Error::IdMismatch {
            requested: want.id,
            found: other,
        }
    );
}

#[test]
fn a_declared_id_no_key_proves_is_refused() {
    let want = expected();
    let error =
        verify(&fixture("no-developer.crx"), &want.id, &want.publisher).expect_err("refused");
    assert_eq!(error, Crx3Error::NoDeveloperProof);
}

#[test]
fn the_requested_id_must_be_the_files_id() {
    let want = expected();
    let other = "a".repeat(32);
    let error = verify(&fixture("valid.crx"), &other, &want.publisher).expect_err("refused");
    assert_eq!(
        error,
        Crx3Error::IdMismatch {
            requested: other,
            found: want.id,
        }
    );
}

#[test]
fn a_truncated_file_is_refused() {
    let want = expected();
    let error = verify(&fixture("truncated.crx"), &want.id, &want.publisher).expect_err("refused");
    assert_eq!(error, Crx3Error::Truncated);
}

#[test]
fn without_the_publisher_proof_it_is_refused() {
    let want = expected();
    let error =
        verify(&fixture("no-publisher.crx"), &want.id, &want.publisher).expect_err("refused");
    assert_eq!(error, Crx3Error::NoPublisherProof);
    // The fixtures are not signed by the Web Store: against Google's key they are refused too.
    let error = verify(
        &fixture("valid.crx"),
        &want.id,
        &WEB_STORE_PUBLISHER_KEY_HASH,
    )
    .expect_err("refused");
    assert_eq!(error, Crx3Error::NoPublisherProof);
}

#[test]
fn a_zip_slip_crx_still_verifies_the_unpacking_refuses_it() {
    let want = expected();
    assert!(verify(&fixture("zip-slip.crx"), &want.id, &want.publisher).is_ok());
}

#[test]
fn not_a_crx_is_refused_by_its_magic_or_version() {
    let want = expected();
    assert_eq!(
        verify(
            b"PK\x03\x04 not a crx at all, just a zip",
            &want.id,
            &want.publisher
        )
        .expect_err("refused"),
        Crx3Error::BadMagic
    );
    let mut bytes = fixture("valid.crx");
    bytes[4] = 2;
    assert_eq!(
        verify(&bytes, &want.id, &want.publisher).expect_err("refused"),
        Crx3Error::BadVersion(2)
    );
    assert_eq!(
        verify(b"Cr24", &want.id, &want.publisher).expect_err("refused"),
        Crx3Error::Truncated
    );
}

#[test]
fn a_header_larger_than_the_limit_is_refused_before_it_is_read() {
    let want = expected();
    let mut bytes = b"Cr24\x03\x00\x00\x00".to_vec();
    bytes.extend_from_slice(&(MAX_HEADER_BYTES + 1).to_le_bytes());
    assert_eq!(
        verify(&bytes, &want.id, &want.publisher).expect_err("refused"),
        Crx3Error::HeaderTooLarge
    );
}

#[test]
fn every_error_has_a_stable_code() {
    let errors = [
        Crx3Error::BadMagic,
        Crx3Error::BadVersion(2),
        Crx3Error::HeaderTooLarge,
        Crx3Error::Truncated,
        Crx3Error::BadHeader("x".to_owned()),
        Crx3Error::NoDeveloperProof,
        Crx3Error::UnsupportedKey,
        Crx3Error::IdMismatch {
            requested: String::new(),
            found: String::new(),
        },
        Crx3Error::BadSignature,
        Crx3Error::NoPublisherProof,
    ];
    for error in errors {
        assert!(error.code().starts_with("extensions.crx_"), "{error:?}");
    }
}
