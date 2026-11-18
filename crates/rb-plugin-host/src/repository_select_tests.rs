use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use super::{Reachability, Shipped, UNREACHABLE_FOR, newer_than, replacement};
use crate::index::fixtures::entry;
use crate::index::{Compatibility, IndexEntry, Offer};

fn offer(repository: &str, version: &str, fingerprint: &str, capabilities: &[&str]) -> Offer {
    let mut entry: IndexEntry = entry("axe", version);
    entry.publisher.fingerprint = fingerprint.to_owned();
    entry.capabilities = capabilities.iter().map(|id| (*id).to_owned()).collect();
    Offer {
        repository: repository.to_owned(),
        entry,
        compatibility: Compatibility::Compatible,
    }
}

fn shipped(version: &str) -> Shipped {
    Shipped {
        id: "axe".to_owned(),
        version: semver::Version::parse(version).expect("version"),
        fingerprint: "f".repeat(64),
        capabilities: BTreeSet::from(["cdp".to_owned(), "views".to_owned()]),
    }
}

/// RD-1260-02: the repository's version stands in when it is newer than the bundle's, signed by
/// the same key and asking for nothing more; the newest such wins.
#[test]
fn the_newest_offer_of_the_same_key_and_no_more_grants_is_taken() {
    let key = "f".repeat(64);
    let offers = vec![
        offer("official", "1.0.0", &key, &["cdp"]),
        offer("official", "1.2.0", &key, &["cdp", "views"]),
        offer("mine", "1.1.0", &key, &["views"]),
    ];
    let taken = replacement(&offers, &shipped("1.0.0")).expect("one");
    assert_eq!(taken.entry.version, "1.2.0");
}

#[test]
fn an_older_offer_another_key_or_more_grants_leaves_the_bundle() {
    let key = "f".repeat(64);
    let other = "e".repeat(64);
    assert!(replacement(&[offer("official", "0.9.0", &key, &[])], &shipped("1.0.0")).is_none());
    // The bundle's own version installs from the bundle: the loader lets only a newer copy stand
    // in, so downloading it would be wasted (review 5).
    assert!(replacement(&[offer("official", "1.0.0", &key, &[])], &shipped("1.0.0")).is_none());
    assert!(
        replacement(
            &[offer("official", "2.0.0", &other, &[])],
            &shipped("1.0.0")
        )
        .is_none()
    );
    assert!(
        replacement(
            &[offer("official", "2.0.0", &key, &["cdp", "http"])],
            &shipped("1.0.0")
        )
        .is_none(),
        "asks for http on top"
    );
    let mut incompatible = offer("official", "2.0.0", &key, &[]);
    incompatible.compatibility = Compatibility::ApiVersion {
        api_version: "9.0.0".to_owned(),
    };
    assert!(replacement(&[incompatible], &shipped("1.0.0")).is_none());
}

/// An update of an installed plugin is strictly newer, under the same rule.
#[test]
fn an_update_is_strictly_newer() {
    let key = "f".repeat(64);
    let offers = vec![offer("official", "1.0.0", &key, &["cdp"])];
    assert!(newer_than(&offers, &shipped("1.0.0")).is_none());
    let offers = vec![offer("official", "1.0.1", &key, &["cdp"])];
    assert_eq!(
        newer_than(&offers, &shipped("1.0.0"))
            .expect("one")
            .entry
            .version,
        "1.0.1"
    );
}

/// A host that failed is not asked for five minutes; one that answered is asked again at once.
#[test]
fn a_failed_host_rests_for_five_minutes() {
    let reachability = Reachability::default();
    let url = url::Url::parse("https://github.com/x/rbrowser-plugin-index.json").expect("url");
    let now = Instant::now();
    assert!(reachability.reachable(&url, now));
    reachability.note(&url, false, now);
    assert!(!reachability.reachable(&url, now + Duration::from_secs(60)));
    assert!(reachability.reachable(&url, now + UNREACHABLE_FOR));
    reachability.note(&url, true, now);
    assert!(reachability.reachable(&url, now));
}

/// Review 6: an update installed in this session is the base of the next check, so the same
/// update is not offered (or downloaded) again before the restart.
#[test]
fn a_version_installed_this_session_raises_the_base() {
    let key = "f".repeat(64);
    let offers = vec![offer("official", "1.1.0", &key, &["cdp"])];
    assert!(newer_than(&offers, &shipped("1.0.0")).is_some());
    let pending = shipped("1.0.0").at_least("1.1.0");
    assert!(newer_than(&offers, &pending).is_none());
    assert_eq!(
        shipped("1.2.0").at_least("1.1.0").version.to_string(),
        "1.2.0"
    );
    assert_eq!(
        shipped("1.0.0").at_least("garbage").version.to_string(),
        "1.0.0"
    );
}
