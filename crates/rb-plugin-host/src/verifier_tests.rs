use rb_plugin_types::codes;

use super::{PluginVerifier, VerifyError};
use crate::test_support::{KEY_ID, ManifestBuilder, signing_key, tampered};
use crate::testbed::{self, Bed};
use crate::{COMPONENT_MEMBER, MANIFEST_MEMBER};

fn trusting(seed: u8) -> PluginVerifier {
    let verifier = PluginVerifier::new(false);
    verifier
        .trust_key(KEY_ID.to_owned(), signing_key(seed).verifying_key())
        .expect("trust");
    verifier
}

#[test]
fn a_signed_package_by_a_trusted_key_verifies() {
    let key = signing_key(1);
    let package = ManifestBuilder::default().package(&Bed::feature().build(), &key, true, &[]);
    let verified = trusting(1).verify_bytes(&package).expect("verifies");
    assert_eq!(verified.manifest.id, "sample");
    assert!(verified.signature.is_some());
}

#[test]
fn a_changed_package_is_refused_with_an_understandable_reason() {
    let key = signing_key(1);
    let package = ManifestBuilder::default().package(&Bed::feature().build(), &key, true, &[]);
    for (member, change) in [
        (
            COMPONENT_MEMBER,
            (|bytes: &mut Vec<u8>| bytes.push(0)) as fn(&mut Vec<u8>),
        ),
        (MANIFEST_MEMBER, |bytes: &mut Vec<u8>| {
            bytes.extend_from_slice(b"\n# changed\n");
        }),
        ("locales/en.json", |bytes: &mut Vec<u8>| {
            *bytes = br#"{"plugin": {"sample": {"name": "Changed"}}}"#.to_vec();
        }),
    ] {
        let changed = tampered(&package, member, change);
        let error = trusting(1).verify_bytes(&changed).expect_err(member);
        assert_eq!(error.code(), codes::SIGNATURE_INVALID, "{member}");
        let reason = format!("{error:#}");
        assert!(
            reason.contains("changed after it was signed"),
            "the reason says what happened: {reason}"
        );
    }
}

#[test]
fn an_unknown_key_is_a_decision_with_its_fingerprint() {
    let key = signing_key(2);
    let package = ManifestBuilder::default().package(&Bed::feature().build(), &key, true, &[]);
    match PluginVerifier::new(false).verify_bytes(&package) {
        Err(VerifyError::UntrustedKey {
            key_id,
            fingerprint,
            ..
        }) => {
            assert_eq!(key_id, KEY_ID);
            assert_eq!(fingerprint.len(), 64);
        }
        other => panic!("expected an untrusted key, got {other:?}"),
    }
    let mismatch = trusting(3).verify_bytes(&package).expect_err("other key");
    assert_eq!(mismatch.code(), codes::KEY_MISMATCH);
}

#[test]
fn an_unsigned_package_needs_development_mode() {
    let key = signing_key(1);
    let package = ManifestBuilder::default().package(&Bed::feature().build(), &key, false, &[]);
    let error = trusting(1).verify_bytes(&package).expect_err("unsigned");
    assert_eq!(error.code(), codes::UNSIGNED);
    PluginVerifier::new(true)
        .verify_bytes(&package)
        .expect("development mode accepts it");
}

#[test]
fn a_component_reaching_beyond_its_grants_is_refused_at_install() {
    let key = signing_key(1);
    let http = ManifestBuilder::default().package_unchecked(&testbed::importing_http(), &key);
    let error = trusting(1)
        .verify_bytes(&http)
        .expect_err("http not granted");
    assert_eq!(error.code(), codes::CAPABILITY_DENIED);
    assert!(format!("{error:#}").contains("capability=http"));
    let wasi = ManifestBuilder::default().package_unchecked(&testbed::importing_wasi(), &key);
    assert_eq!(
        trusting(1).verify_bytes(&wasi).expect_err("wasi").code(),
        codes::DENIED
    );
}

#[test]
fn a_component_without_its_declared_world_is_refused() {
    let key = signing_key(1);
    let builder = ManifestBuilder::default().kinds(&["check-plugin"]);
    let package = builder.package_unchecked(&Bed::feature().build(), &key);
    assert_eq!(
        trusting(1)
            .verify_bytes(&package)
            .expect_err("no check")
            .code(),
        codes::WORLD_MISSING
    );
}

#[test]
fn a_settings_schema_with_a_new_top_level_category_refuses_the_package() {
    let key = signing_key(1);
    let builder =
        ManifestBuilder::default().tables("\n[settings]\nschema = \"settings.schema.json\"\n");
    let schema = br#"{"type": "object", "properties": {"depth": {"type": "integer",
        "x-category": "sample-tools", "x-section": "general", "x-order": 1}}}"#;
    let package = crate::package_plugin(
        builder.text(&key).as_bytes(),
        &Bed::feature().build(),
        &[
            builder.locale(),
            ("settings.schema.json".to_owned(), schema.to_vec()),
        ],
        Some(&key),
    )
    .expect_err("the packager refuses it too");
    assert_eq!(crate::refusal_code(&package), codes::SETTINGS_CATEGORY);
}
