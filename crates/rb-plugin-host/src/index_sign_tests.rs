use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

use super::{
    Freshness, IndexSigners, SignatureError, SignedDocument, StaleError, check_freshness,
    sign_document,
};
use crate::sign::TrustStore;

const DOMAIN: &str = "rbrowser.test-document.v1";

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct Payload {
    name: String,
    count: u32,
}

fn payload() -> Payload {
    Payload {
        name: "catalogue".to_owned(),
        count: 3,
    }
}

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn roots(seed: u8) -> IndexSigners {
    let store = TrustStore::new();
    store
        .trust("root-v1".to_owned(), key(seed).verifying_key())
        .expect("trust");
    IndexSigners::Roots(store)
}

fn signed(seed: u8, key_id: &str) -> Vec<u8> {
    let document = sign_document(DOMAIN, key_id, &key(seed), &payload()).expect("sign");
    serde_json::to_vec(&document).expect("encode")
}

#[test]
fn a_signed_document_round_trips_under_its_root() {
    let bytes = signed(1, "root-v1");
    let document = SignedDocument::parse(&bytes).expect("parse");
    let read: Payload = document.verify(DOMAIN, &roots(1)).expect("verify");
    assert_eq!(read, payload());
}

#[test]
fn a_pinned_key_verifies_whatever_key_id_the_document_names() {
    let bytes = signed(4, "anything-v7");
    let document = SignedDocument::parse(&bytes).expect("parse");
    let pinned = IndexSigners::Pinned(key(4).verifying_key());
    let read: Payload = document.verify(DOMAIN, &pinned).expect("verify");
    assert_eq!(read, payload());
    let other = IndexSigners::Pinned(key(5).verifying_key());
    assert!(matches!(
        document.verify::<Payload>(DOMAIN, &other),
        Err(SignatureError::BadSignature { .. })
    ));
}

#[test]
fn a_changed_payload_is_a_bad_signature() {
    let text = String::from_utf8(signed(1, "root-v1")).expect("utf-8");
    let changed = text.replacen("\"count\":3", "\"count\":4", 1);
    assert_ne!(text, changed, "the fixture holds the field");
    let document = SignedDocument::parse(changed.as_bytes()).expect("parse");
    assert!(matches!(
        document.verify::<Payload>(DOMAIN, &roots(1)),
        Err(SignatureError::BadSignature { .. })
    ));
}

#[test]
fn a_signature_for_another_domain_does_not_hold() {
    let document = sign_document(
        "rbrowser.update-manifest.v1",
        "root-v1",
        &key(1),
        &payload(),
    )
    .expect("sign");
    assert!(matches!(
        document.verify::<Payload>(DOMAIN, &roots(1)),
        Err(SignatureError::BadSignature { .. })
    ));
}

#[test]
fn an_unknown_key_id_is_untrusted_and_an_impostor_under_the_root_id_is_a_bad_signature() {
    let unknown = SignedDocument::parse(&signed(1, "stranger")).expect("parse");
    assert!(matches!(
        unknown.verify::<Payload>(DOMAIN, &roots(1)),
        Err(SignatureError::Untrusted { .. })
    ));
    let impostor = SignedDocument::parse(&signed(9, "root-v1")).expect("parse");
    assert!(matches!(
        impostor.verify::<Payload>(DOMAIN, &roots(1)),
        Err(SignatureError::BadSignature { .. })
    ));
}

#[test]
fn another_algorithm_does_not_count() {
    let text = String::from_utf8(signed(1, "root-v1")).expect("utf-8");
    let other = text.replacen("\"ed25519\"", "\"rsa\"", 1);
    let document = SignedDocument::parse(other.as_bytes()).expect("parse");
    assert!(matches!(
        document.verify::<Payload>(DOMAIN, &roots(1)),
        Err(SignatureError::Untrusted { .. })
    ));
}

#[test]
fn too_many_signatures_do_not_parse() {
    let mut value: serde_json::Value = serde_json::from_slice(&signed(1, "root-v1")).expect("json");
    let one = value["signatures"][0].clone();
    value["signatures"] = serde_json::Value::Array(vec![one; 9]);
    let bytes = serde_json::to_vec(&value).expect("encode");
    assert!(SignedDocument::parse(&bytes).is_err());
}

fn at(minutes: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000 + minutes * 60, 0).expect("timestamp")
}

fn fresh(sequence: u64) -> Freshness {
    Freshness {
        sequence,
        issued_at: at(0),
        expires_at: at(60),
    }
}

#[test]
fn a_newer_sequence_is_fresh_and_the_same_or_an_older_one_a_replay() {
    assert_eq!(check_freshness(fresh(8), Some(7), at(1)), Ok(()));
    for older in [7, 6] {
        assert_eq!(
            check_freshness(fresh(older), Some(7), at(1)),
            Err(StaleError::Replayed {
                saw: older,
                known: 7
            })
        );
    }
}

#[test]
fn a_first_contact_takes_any_sequence_but_never_an_expired_index() {
    assert_eq!(check_freshness(fresh(1), None, at(1)), Ok(()));
    assert!(matches!(
        check_freshness(fresh(1), None, at(61)),
        Err(StaleError::Expired { .. })
    ));
}

#[test]
fn clock_skew_is_tolerated_and_next_week_is_not() {
    let skewed = Freshness {
        issued_at: at(30),
        ..fresh(1)
    };
    assert_eq!(check_freshness(skewed, None, at(0)), Ok(()));
    let ahead = Freshness {
        issued_at: at(0) + Duration::days(7),
        expires_at: at(0) + Duration::days(30),
        sequence: 1,
    };
    assert!(matches!(
        check_freshness(ahead, None, at(0)),
        Err(StaleError::FromTheFuture { .. })
    ));
}
