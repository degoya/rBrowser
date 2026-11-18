//! The beta channel finds its manifest through the release list; the list only locates, the
//! signature decides, and a signed manifest still cannot send a download elsewhere.

use serde_json::json;

use super::*;
use crate::MemoryFetcher;
use crate::manifest::tests::{PREFIX, at, key, manifest, signed, trust_for};

const STABLE: &str =
    "https://github.com/degoya/rBrowser/releases/latest/download/rbrowser-update-stable.json";
const LIST: &str = "https://api.github.com/repos/degoya/rBrowser/releases?per_page=30";

fn beta_url(version: &str) -> String {
    format!("{PREFIX}v{version}/rbrowser-update-beta.json")
}

fn release(tag: &str, url: &str) -> serde_json::Value {
    json!({
        "tag_name": tag,
        "draft": false,
        "prerelease": tag.contains('-'),
        "assets": [
            { "name": "rbrowser-0.9.0-windows-x64.zip", "browser_download_url": format!("{PREFIX}{tag}/rbrowser-0.9.0-windows-x64.zip") },
            { "name": "rbrowser-update-beta.json", "browser_download_url": url },
        ],
    })
}

fn list(releases: &[serde_json::Value]) -> Vec<u8> {
    serde_json::to_vec(releases).expect("json")
}

#[test]
fn the_official_sources_are_the_repositorys_latest_release_and_its_release_list() {
    let sources = Sources::official();
    assert_eq!(sources.stable.as_str(), STABLE);
    assert_eq!(sources.release_list.as_str(), LIST);
    assert_eq!(sources.download_prefix, PREFIX);
    assert_eq!(
        Sources::release_page("0.9.0-beta.2"),
        "https://github.com/degoya/rBrowser/releases/tag/v0.9.0-beta.2"
    );
}

#[test]
fn the_list_locates_the_highest_beta_under_the_repositorys_own_downloads() {
    let releases = list(&[
        release("v0.9.0-beta.2", &beta_url("0.9.0-beta.2")),
        release("v0.9.0-beta.10", &beta_url("0.9.0-beta.10")),
        // Higher, but its asset lies elsewhere: never located.
        release(
            "v0.9.0-beta.11",
            "https://example.test/rbrowser-update-beta.json",
        ),
        json!({ "tag_name": "v0.9.0-beta.12", "draft": true, "assets": [
            { "name": "rbrowser-update-beta.json", "browser_download_url": beta_url("0.9.0-beta.12") }
        ] }),
        json!({ "tag_name": "not-a-version", "assets": [] }),
    ]);
    let url = beta_manifest_url(&releases, PREFIX)
        .expect("a list")
        .expect("a beta");
    assert_eq!(url.as_str(), beta_url("0.9.0-beta.10"));
    assert!(matches!(
        beta_manifest_url(b"{\"message\":\"rate limited\"}", PREFIX),
        Err(UpdateError::Fetch(_))
    ));
    assert_eq!(beta_manifest_url(b"[]", PREFIX).expect("a list"), None);
}

#[test]
fn the_beta_channel_reads_the_stable_manifest_and_the_located_beta() {
    let data = tempfile::tempdir().expect("tempdir");
    let fetcher = MemoryFetcher::new();
    fetcher.serve(STABLE, signed(&manifest(Channel::Stable, "0.8.0", 3)));
    fetcher.serve(
        LIST,
        list(&[release("v0.9.0-beta.2", &beta_url("0.9.0-beta.2"))]),
    );
    fetcher.serve(
        &beta_url("0.9.0-beta.2"),
        signed(&manifest(Channel::Beta, "0.9.0-beta.2", 5)),
    );
    let trust = trust_for(&key());
    let sources = Sources::official();
    let report = check_channel_with(
        &trust,
        &fetcher,
        &sources,
        Channel::Beta,
        data.path(),
        at(1),
    );
    assert!(report.problem.is_none(), "{:?}", report.problem);
    let versions: Vec<&str> = report
        .manifests
        .iter()
        .map(|m| m.version.as_str())
        .collect();
    assert_eq!(versions, ["0.8.0", "0.9.0-beta.2"]);
    let floors = Floors::load(data.path());
    assert_eq!(floors.get(Channel::Stable), Some(3));
    assert_eq!(floors.get(Channel::Beta), Some(5));

    // The stable channel never asks for the list.
    let stable = MemoryFetcher::new();
    stable.serve(STABLE, signed(&manifest(Channel::Stable, "0.8.0", 3)));
    let report = check_channel_with(
        &trust,
        &stable,
        &sources,
        Channel::Stable,
        data.path(),
        at(1),
    );
    assert_eq!(report.manifests.len(), 1);
    assert_eq!(stable.requests(), [STABLE]);
}

/// Only pre-releases are published (0.9.0-beta.N): `latest` answers 404, which is no problem
/// while the beta verified.
#[test]
fn no_stable_release_yet_is_quiet_on_the_beta_channel_and_reported_on_stable() {
    let data = tempfile::tempdir().expect("tempdir");
    let fetcher = MemoryFetcher::new();
    fetcher.serve(
        LIST,
        list(&[release("v0.9.0-beta.2", &beta_url("0.9.0-beta.2"))]),
    );
    fetcher.serve(
        &beta_url("0.9.0-beta.2"),
        signed(&manifest(Channel::Beta, "0.9.0-beta.2", 5)),
    );
    let trust = trust_for(&key());
    let sources = Sources::official();
    let beta = check_channel_with(
        &trust,
        &fetcher,
        &sources,
        Channel::Beta,
        data.path(),
        at(1),
    );
    assert_eq!(beta.manifests.len(), 1);
    assert!(beta.problem.is_none());
    let stable = check_channel_with(
        &trust,
        &fetcher,
        &sources,
        Channel::Stable,
        data.path(),
        at(1),
    );
    assert!(stable.manifests.is_empty());
    assert_eq!(
        stable.problem.map(|problem| problem.code()),
        Some("update.not_published")
    );
}

/// A forged beta beside a good stable manifest: the stable one is handed on, the forgery is the
/// problem the window reports.
#[test]
fn a_forged_manifest_is_the_problem_even_when_another_verified() {
    let data = tempfile::tempdir().expect("tempdir");
    let fetcher = MemoryFetcher::new();
    fetcher.serve(STABLE, signed(&manifest(Channel::Stable, "0.8.0", 3)));
    fetcher.serve(
        LIST,
        list(&[release("v0.9.0-beta.2", &beta_url("0.9.0-beta.2"))]),
    );
    let mut forged = signed(&manifest(Channel::Beta, "0.9.0-beta.2", 5));
    let text = String::from_utf8(forged.clone()).expect("utf-8");
    forged = text.replace("0.9.0-beta.2", "0.9.0-beta.9").into_bytes();
    fetcher.serve(&beta_url("0.9.0-beta.2"), forged);
    let report = check_channel_with(
        &trust_for(&key()),
        &fetcher,
        &Sources::official(),
        Channel::Beta,
        data.path(),
        at(1),
    );
    assert_eq!(report.manifests.len(), 1);
    assert_eq!(
        report.problem.map(|problem| problem.code()),
        Some("update.bad_signature")
    );
    assert_eq!(Floors::load(data.path()).get(Channel::Beta), None);
}

#[test]
fn a_signed_manifest_whose_artifact_lies_elsewhere_is_refused() {
    let mut offered = manifest(Channel::Stable, "1.1.0", 1);
    offered.artifacts[0].url = "https://example.test/rbrowser-1.1.0-windows-x86_64.zip".to_owned();
    let error = pinned(offered, PREFIX).expect_err("elsewhere");
    assert_eq!(error.code(), "update.invalid");
    let mut sneaky = manifest(Channel::Stable, "1.1.0", 1);
    sneaky.artifacts[0].url = format!("{PREFIX}../../other/x.zip");
    assert!(
        pinned(sneaky, PREFIX).is_err(),
        "dot segments resolve first"
    );
    assert!(pinned(manifest(Channel::Stable, "1.1.0", 1), PREFIX).is_ok());
}

#[test]
fn a_shut_gate_makes_no_request() {
    let data = tempfile::tempdir().expect("tempdir");
    let fetcher = MemoryFetcher::new();
    let gate = Gate {
        feature_built: true,
        enabled: false,
    };
    let error = check_channel(
        gate,
        &fetcher,
        &Sources::official(),
        Channel::Beta,
        data.path(),
        at(1),
    )
    .expect_err("shut");
    assert_eq!(error.code(), "update.disabled");
    assert!(fetcher.requests().is_empty());
}

/// 180 days without a release: the manifest expired. Nothing is offered and the problem says
/// "expired", which the app logs without a notice (RB-090-14).
#[test]
fn an_expired_manifest_offers_nothing_and_is_no_integrity_problem() {
    let data = tempfile::tempdir().expect("tempdir");
    let fetcher = MemoryFetcher::new();
    fetcher.serve(STABLE, signed(&manifest(Channel::Stable, "0.8.0", 3)));
    let report = check_channel_with(
        &trust_for(&key()),
        &fetcher,
        &Sources::official(),
        Channel::Stable,
        data.path(),
        at(200),
    );
    assert!(report.manifests.is_empty());
    let problem = report.problem.expect("a problem");
    assert!(problem.is_expired(), "{problem}");
    assert!(!problem.is_integrity());
}
