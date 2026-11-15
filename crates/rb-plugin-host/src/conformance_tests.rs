use super::run_conformance;
use crate::PluginVerifier;
use crate::test_support::{KEY_ID, ManifestBuilder, signing_key};
use crate::testbed::Bed;

fn verifier() -> PluginVerifier {
    let verifier = PluginVerifier::new(false);
    verifier
        .trust_key(KEY_ID.to_owned(), signing_key(1).verifying_key())
        .expect("trust");
    verifier
}

#[tokio::test]
async fn a_bed_of_every_kind_passes() {
    for (kind, bed) in [
        ("feature-plugin", Bed::feature()),
        ("check-plugin", Bed::check()),
        ("analyzer-plugin", Bed::analyzer()),
        ("export-plugin", Bed::exporter()),
    ] {
        let builder = ManifestBuilder::default().kinds(&[kind]);
        let package = builder.package(&bed.build(), &signing_key(1), true, &[]);
        let report = run_conformance(&package, &verifier()).await;
        assert!(report.passed, "{kind}: {report:?}");
        assert_eq!(
            report.checks.len(),
            3,
            "package, world and the kind's question"
        );
    }
}

#[tokio::test]
async fn a_hanging_plugin_fails_its_question() {
    let builder = ManifestBuilder::default().limit("fuel = 100000");
    let package = builder.package(
        &Bed::feature().endless("activate").build(),
        &signing_key(1),
        true,
        &[],
    );
    let report = run_conformance(&package, &verifier()).await;
    assert!(!report.passed);
    let failed = report
        .checks
        .iter()
        .find(|check| !check.passed)
        .expect("one failed");
    assert_eq!(failed.id, "feature-plugin");
}

#[tokio::test]
async fn an_unverifiable_package_stops_at_the_package_check() {
    let report = run_conformance(b"not a zip", &verifier()).await;
    assert!(!report.passed);
    assert_eq!(report.checks.len(), 1);
    assert_eq!(report.checks[0].id, "package");
}
