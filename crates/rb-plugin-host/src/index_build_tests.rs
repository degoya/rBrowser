use super::super::fixtures::{index_of, now, roots, signed};
use super::{describe_package, package_file_name};
use crate::PluginVerifier;
use crate::sign::{hex_sha256, key_fingerprint};
use crate::test_support::{KEY_ID, ManifestBuilder, signing_key};
use crate::testbed::Bed;

#[test]
fn an_entry_describes_the_verified_package_and_signs_into_an_index_that_verifies() {
    let key = signing_key(1);
    let file = ManifestBuilder::default()
        .capability("views = true")
        .package(&Bed::feature().build(), &key, true, &[]);
    let verifier = PluginVerifier::new(false);
    verifier
        .trust_key(KEY_ID.to_owned(), key.verifying_key())
        .expect("trust");
    let package = verifier.verify_bytes(&file).expect("verifies");
    let name = package_file_name("sample", "1.0.0");
    assert_eq!(name, "sample-1.0.0.rbplugin");
    let entry = describe_package(&file, &package, &name, None).expect("describe");
    assert_eq!(entry.id, "sample");
    assert_eq!(entry.version, "1.0.0");
    assert_eq!(entry.kinds, ["feature-plugin"]);
    assert_eq!(entry.capabilities, ["views"]);
    assert_eq!(entry.sha256, hex_sha256(&file));
    assert_eq!(entry.size, u64::try_from(file.len()).expect("size"));
    assert_eq!(entry.publisher.key_id, KEY_ID);
    assert_eq!(
        entry.publisher.fingerprint,
        key_fingerprint(&key.verifying_key())
    );
    let bytes = signed(&index_of(1, vec![entry.clone()]));
    let verified = super::super::verify(&bytes, &roots(), None, now()).expect("verify");
    assert_eq!(verified.plugins, [entry]);
}

/// RB-091-04: the index keeps the newest version of each plugin per contract it no longer
/// builds for; what the release builds replaces its own contract's entries.
#[test]
fn the_index_carries_the_last_version_of_every_other_contract() {
    use super::super::fixtures::entry;
    let absolute = |id: &str, version: &str, api: &str| {
        let mut entry = entry(id, version);
        entry.api_version = api.to_owned();
        entry.url = format!("https://example.com/plugins-v{version}/{id}-{version}.rbplugin");
        entry
    };
    let current = vec![absolute("axe", "2.0.0", "2.0.0")];
    let previous = vec![
        absolute("axe", "1.0.0", "1.0.0"),
        absolute("axe", "1.1.0", "1.0.0"),
        absolute("axe", "1.9.0", "2.0.0"),
        absolute("todo", "1.0.0", "1.0.0"),
        entry("relative", "1.0.0"),
    ];
    let index = super::carry_over(&previous, current);
    let listed: Vec<(&str, &str)> = index
        .iter()
        .map(|entry| (entry.id.as_str(), entry.version.as_str()))
        .collect();
    assert_eq!(
        listed,
        [("axe", "2.0.0"), ("axe", "1.1.0"), ("todo", "1.0.0")]
    );
}
