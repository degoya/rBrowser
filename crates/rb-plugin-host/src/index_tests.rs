//! Each refusal of [`super::verify`] in the documented order, with its stable code (RB-091-01:
//! a wrong signature, an expired index and an older sequence are refused).

use ed25519_dalek::SigningKey;
use time::SignedDuration;

use super::fixtures::{entry, index, index_of, now, pinned, roots, signed, signed_raw};
use super::*;

fn refused(bytes: &[u8], known: Option<u64>) -> IndexError {
    verify(bytes, &roots(), known, now()).expect_err("refused")
}

/// Signs the default index after `edit` changed its JSON, and returns the refusal.
fn refused_after(edit: impl FnOnce(&mut serde_json::Value)) -> IndexError {
    let mut payload = serde_json::to_value(index()).expect("encode");
    edit(&mut payload);
    refused(&signed_raw(&payload), None)
}

/// A change to an index payload, for a case of the table.
type Edit = Box<dyn Fn(&mut serde_json::Value)>;
#[test]
fn a_signed_index_verifies_and_yields_its_plugins() {
    let verified = verify(&signed(&index()), &roots(), Some(4), now()).expect("verify");
    assert_eq!(verified, index());
    let pinned = verify(&signed(&index()), &pinned(), None, now()).expect("verify pinned");
    assert_eq!(pinned.plugins[0].id, "fixture");
}

/// RB-090-77: the index's times are RFC 3339 in UTC with `Z`, as chrono wrote them.
#[test]
fn an_index_writes_its_times_in_utc() {
    let json = serde_json::to_value(index()).expect("json");
    assert_eq!(json["issued_at"], "2026-09-21T13:13:20Z");
    assert_eq!(json["expires_at"], "2026-10-21T14:13:20Z");
}

#[test]
fn a_changed_byte_is_a_bad_signature() {
    let text = String::from_utf8(signed(&index())).expect("utf-8");
    assert!(text.contains("\"1.2.3\""));
    let changed = text.replacen("\"1.2.3\"", "\"1.2.4\"", 1).into_bytes();
    assert_eq!(
        refused(&changed, None).code(),
        "plugins.catalog.error.bad_signature"
    );
}

#[test]
fn another_key_is_refused_whether_or_not_it_claims_the_trusted_id() {
    let impostor = SigningKey::from_bytes(&[3; 32]);
    let claims_our_id = sign(fixtures::REPOSITORY_KEY_ID, &impostor, &index()).expect("sign");
    assert_eq!(
        refused(&claims_our_id, None).code(),
        "plugins.catalog.error.bad_signature"
    );
    let own_id = sign("impostor", &impostor, &index()).expect("sign");
    assert_eq!(
        refused(&own_id, None).code(),
        "plugins.catalog.error.untrusted"
    );
    let against_pin = verify(&own_id, &pinned(), None, now()).expect_err("refused");
    assert_eq!(against_pin.code(), "plugins.catalog.error.bad_signature");
}

#[test]
fn an_expired_index_is_refused_even_on_a_first_contact() {
    let mut old = index();
    old.issued_at = now() - SignedDuration::days(40);
    old.expires_at = now() - SignedDuration::days(1);
    let error = refused(&signed(&old), None);
    assert_eq!(error.code(), "plugins.catalog.error.expired");
}

#[test]
fn an_older_or_the_same_sequence_is_a_replay() {
    let bytes = signed(&index_of(5, vec![entry("fixture", "1.2.3")]));
    assert_eq!(
        refused(&bytes, Some(6)).code(),
        "plugins.catalog.error.replayed"
    );
    assert_eq!(
        refused(&bytes, Some(5)).code(),
        "plugins.catalog.error.replayed"
    );
    assert!(verify(&bytes, &roots(), Some(4), now()).is_ok());
}

#[test]
fn an_index_from_next_week_is_refused() {
    let mut ahead = index();
    ahead.issued_at = now() + SignedDuration::days(7);
    ahead.expires_at = now() + SignedDuration::days(30);
    assert_eq!(
        refused(&signed(&ahead), None).code(),
        "plugins.catalog.error.from_the_future"
    );
}

#[test]
fn too_large_and_unreadable_come_before_the_signature() {
    let huge = vec![b' '; MAX_INDEX_BYTES + 1];
    assert_eq!(
        refused(&huge, None).code(),
        "plugins.catalog.error.too_large"
    );
    assert_eq!(
        refused(b"not json", None).code(),
        "plugins.catalog.error.malformed"
    );
}

#[test]
fn a_future_schema_version_is_refused_after_the_signature_held() {
    let error = refused_after(|payload| payload["schema_version"] = 2.into());
    assert_eq!(error.code(), "plugins.catalog.error.schema_version");
}

/// Forward compatibility (RB-091-04): a field a later index adds - at the top, in an entry or
/// in its publisher - is ignored. The signature is the integrity, the package the rights, so an
/// unknown field can neither change what installs nor what it may do.
#[test]
fn an_unknown_field_is_ignored() {
    let mut payload = serde_json::to_value(index()).expect("encode");
    payload["mirror"] = "https://elsewhere.test".into();
    payload["plugins"][0]["category"] = "checks".into();
    payload["plugins"][0]["screenshots"] = serde_json::json!(["a.png"]);
    payload["plugins"][0]["publisher"]["homepage"] = "https://example.test".into();
    let read = verify(&signed_raw(&payload), &roots(), None, now()).expect("verifies");
    assert_eq!(read.plugins[0].id, "fixture");
    assert_eq!(read.plugins[0].category.as_deref(), Some("checks"));
}

/// The previous index of a release (`rb-plugin-pack index --previous`) is held to its signature
/// only: an index past its expiry still carries its entries over.
#[test]
fn the_signature_alone_reads_an_expired_index() {
    let mut old = index();
    old.issued_at = now() - SignedDuration::days(400);
    old.expires_at = now() - SignedDuration::days(300);
    let bytes = signed_raw(&serde_json::to_value(&old).expect("encode"));
    assert_eq!(
        verify(&bytes, &roots(), None, now())
            .expect_err("expired")
            .code(),
        "plugins.catalog.error.expired"
    );
    let read = verify_signature(&bytes, &pinned()).expect("signed");
    assert_eq!(read.plugins.len(), 1);
    let mut tampered = bytes;
    let at = tampered.len() / 2;
    tampered[at] ^= 1;
    assert!(verify_signature(&tampered, &pinned()).is_err());
}

#[test]
fn content_rules_refuse_the_whole_index() {
    let cases: Vec<(&str, Edit)> = vec![
        (
            "sequence 0",
            Box::new(|p: &mut serde_json::Value| p["sequence"] = 0.into()),
        ),
        (
            "window too long",
            Box::new(|p: &mut serde_json::Value| {
                p["expires_at"] = crate::utc::rfc3339(now() + SignedDuration::days(400)).into();
            }),
        ),
        (
            "bad sha256",
            Box::new(|p: &mut serde_json::Value| p["plugins"][0]["sha256"] = "ABC".into()),
        ),
        (
            "upper-case sha256",
            Box::new(|p: &mut serde_json::Value| {
                p["plugins"][0]["sha256"] = fixtures::SHA256.to_uppercase().into()
            }),
        ),
        (
            "size 0",
            Box::new(|p: &mut serde_json::Value| p["plugins"][0]["size"] = 0.into()),
        ),
        (
            "size beyond any package",
            Box::new(|p: &mut serde_json::Value| {
                p["plugins"][0]["size"] = (crate::package::MAX_PACKAGE_BYTES + 1).into();
            }),
        ),
        (
            "plain http",
            Box::new(|p: &mut serde_json::Value| {
                p["plugins"][0]["url"] = "http://example.test/a.rbplugin".into()
            }),
        ),
        (
            "leaves the directory",
            Box::new(|p: &mut serde_json::Value| {
                p["plugins"][0]["url"] = "../other/a.rbplugin".into()
            }),
        ),
        (
            "credentials",
            Box::new(|p: &mut serde_json::Value| {
                p["plugins"][0]["url"] = "https://user:pw@example.test/a".into()
            }),
        ),
        (
            "no semver",
            Box::new(|p: &mut serde_json::Value| p["plugins"][0]["version"] = "one".into()),
        ),
        (
            "bad min_app_version",
            Box::new(|p: &mut serde_json::Value| {
                p["plugins"][0]["min_app_version"] = "soon".into()
            }),
        ),
        (
            "bad id",
            Box::new(|p: &mut serde_json::Value| p["plugins"][0]["id"] = "Bad_Id".into()),
        ),
        (
            "control characters",
            Box::new(|p: &mut serde_json::Value| p["plugins"][0]["name"] = "a\u{7}b".into()),
        ),
        (
            "capability no name",
            Box::new(|p: &mut serde_json::Value| {
                p["plugins"][0]["capabilities"] = serde_json::json!(["Http!"])
            }),
        ),
    ];
    for (name, edit) in cases {
        let error = refused_after(|payload| edit(payload));
        assert_eq!(error.code(), "plugins.catalog.error.invalid", "{name}");
    }
}

#[test]
fn the_same_plugin_version_twice_is_refused() {
    let twice = index_of(
        3,
        vec![entry("fixture", "1.0.0"), entry("fixture", "1.0.0")],
    );
    let payload = serde_json::to_value(twice).expect("encode");
    assert_eq!(
        refused(&signed_raw(&payload), None).code(),
        "plugins.catalog.error.invalid"
    );
}

#[test]
fn sign_refuses_what_verify_would() {
    let mut broken = index();
    broken.sequence = 0;
    assert!(sign("k", &SigningKey::from_bytes(&[1; 32]), &broken).is_err());
}

#[test]
fn a_relative_url_resolves_below_the_index_and_stays_https() {
    let index_url = url::Url::parse("https://plugins.example.test/repo/rbrowser-plugin-index.json")
        .expect("url");
    let resolved = entry("fixture", "1.2.3")
        .resolve_url(&index_url)
        .expect("resolve");
    assert_eq!(
        resolved.as_str(),
        "https://plugins.example.test/repo/fixture-1.2.3.rbplugin"
    );
    let plain =
        url::Url::parse("http://plugins.example.test/rbrowser-plugin-index.json").expect("url");
    assert!(entry("fixture", "1.2.3").resolve_url(&plain).is_err());
}
