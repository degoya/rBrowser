//! No network call at all while the feature or the setting is off; then the replay floor.

use super::*;
use crate::MemoryFetcher;
use crate::manifest::tests::{at, key, manifest, signed, trust_for};

const BASE: &str = "https://github.com/degoya/rBrowser/releases/download/v1.1.0/";

fn served(sequence: u64) -> MemoryFetcher {
    let fetcher = MemoryFetcher::new();
    fetcher.serve(
        &format!("{BASE}rbrowser-update-stable.json"),
        signed(&manifest(Channel::Stable, "1.1.0", sequence)),
    );
    fetcher
}

/// The acceptance of RB-090-02: off is off - no request, no file, whatever the source serves.
#[test]
fn a_shut_gate_makes_no_request_and_writes_nothing() {
    for gate in [
        Gate {
            feature_built: false,
            enabled: false,
        },
        Gate {
            feature_built: false,
            enabled: true,
        },
        Gate {
            feature_built: true,
            enabled: false,
        },
    ] {
        let data = tempfile::tempdir().expect("tempdir");
        let fetcher = served(1);
        let error =
            check(gate, &fetcher, BASE, Channel::Stable, data.path(), at(1)).expect_err("disabled");
        assert_eq!(error.code(), "update.disabled");
        assert!(fetcher.requests().is_empty(), "{gate:?} made a request");
        assert_eq!(
            std::fs::read_dir(data.path()).expect("list").count(),
            0,
            "{gate:?} wrote a file"
        );
    }
}

/// The shipped root is the owner's key (RB-090-14): an open gate asks once, and a manifest a
/// test key signed is refused without storing a floor.
#[test]
fn an_open_gate_checks_against_the_shipped_root_and_refuses_a_test_key() {
    let data = tempfile::tempdir().expect("tempdir");
    let fetcher = served(1);
    let gate = Gate {
        feature_built: true,
        enabled: true,
    };
    let error =
        check(gate, &fetcher, BASE, Channel::Stable, data.path(), at(1)).expect_err("test key");
    assert_eq!(error.code(), "update.bad_signature", "{error}");
    assert_eq!(fetcher.requests().len(), 1);
    assert_eq!(Floors::load(data.path()).get(Channel::Stable), None);
}

#[test]
fn a_verified_manifest_raises_the_stored_floor_and_an_older_one_is_a_replay() {
    let data = tempfile::tempdir().expect("tempdir");
    let trust = trust_for(&key());
    let newer = served(7);
    let read = check_with(&trust, &newer, BASE, Channel::Stable, data.path(), at(1)).expect("ok");
    assert_eq!(read.sequence, 7);
    assert_eq!(Floors::load(data.path()).get(Channel::Stable), Some(7));
    let older = served(6);
    let error =
        check_with(&trust, &older, BASE, Channel::Stable, data.path(), at(2)).expect_err("replay");
    assert_eq!(error.code(), "update.stale");
    assert_eq!(Floors::load(data.path()).get(Channel::Stable), Some(7));
}

#[test]
fn a_missing_manifest_and_a_plain_http_base_are_refused() {
    let data = tempfile::tempdir().expect("tempdir");
    let trust = trust_for(&key());
    let empty = MemoryFetcher::new();
    let error = check_with(&trust, &empty, BASE, Channel::Beta, data.path(), at(1))
        .expect_err("not published");
    assert_eq!(error.code(), "update.not_published");
    let error = check_with(
        &trust,
        &empty,
        "http://example.test/",
        Channel::Stable,
        data.path(),
        at(1),
    )
    .expect_err("http");
    assert_eq!(error.code(), "update.invalid");
    assert_eq!(empty.requests().len(), 1, "only the https request went out");
}
