//! The loads the hub gives, the views' documents (epochs) and the leader-only submit that
//! stays on its URL.

use std::time::Instant;

use super::tests::{A, B, C, click, hub, submit, targets};
use super::*;
use crate::loading::LOAD_START_GRACE;

#[test]
fn a_resync_to_the_url_the_view_shows_ends_with_its_load() {
    // A desynced view resyncs to the leader's URL - the one it already shows: its address
    // does not change, so no navigation commits; the end of its load gives it events again.
    let mut hub = hub();
    let now = Instant::now();
    hub.record(C, &click().event, &ReplayOutcome::NotFound);
    assert_eq!(hub.resync(C, now), Ok(Some(A)));
    assert_eq!(targets(&hub.route(A, &click(), now)), vec![B]);
    assert!(
        hub.load_state(C, (true, 0), "http://localhost/", now)
            .is_empty()
    );
    assert_eq!(targets(&hub.route(A, &click(), now)), vec![B]);
    assert!(
        hub.load_state(C, (false, 1), "http://localhost/", now)
            .is_empty()
    );
    assert_eq!(targets(&hub.route(A, &click(), now)), vec![B, C]);
}

#[test]
fn a_load_the_hub_gave_that_never_starts_ends_after_the_grace() {
    let mut hub = hub();
    let now = Instant::now();
    hub.mark_loading(B, now);
    assert_eq!(targets(&hub.route(A, &click(), now)), vec![C]);
    let later = now + LOAD_START_GRACE;
    assert!(!hub.is_loading(B, later));
    assert_eq!(targets(&hub.route(A, &click(), later)), vec![B, C]);
}

#[test]
fn a_navigation_that_failed_gives_the_view_its_events_back() {
    let mut hub = hub();
    let now = Instant::now();
    assert_eq!(hub.resync(B, now), Ok(Some(A)));
    hub.mark_loading(C, now);
    hub.load_failed(B);
    hub.load_failed(C);
    assert_eq!(targets(&hub.route(A, &click(), now)), vec![B, C]);
}

#[test]
fn an_outcome_of_a_document_the_view_left_does_not_desync_it() {
    let mut hub = hub();
    let now = Instant::now();
    let routed = hub.epoch(B);
    // B navigates while the replay is under way; its late outcome is of the old document.
    hub.navigated(B, "http://localhost/next", now);
    assert!(!hub.record_at(B, routed, &click().event, &ReplayOutcome::Timeout));
    assert!(hub.desynced().is_empty());
    // A resync starts a new document too.
    let routed = hub.epoch(C);
    hub.resync(C, now).expect("known");
    assert!(!hub.record_at(C, routed, &click().event, &ReplayOutcome::NotFound));
    assert!(hub.desynced().is_empty());
    // The outcome of the current document counts.
    let current = hub.epoch(C);
    assert!(hub.record_at(C, current, &click().event, &ReplayOutcome::NotFound));
    assert_eq!(hub.desynced(), vec![C]);
}

#[test]
fn a_leader_only_submit_that_stays_on_its_url_has_the_followers_load_it() {
    // A form posted to its own page: the leader's address does not change, no navigation
    // commits; the end of its load is what the followers wait for.
    let mut hub = hub();
    let now = Instant::now();
    assert!(matches!(hub.route(A, &submit(), now), Route::Follow { .. }));
    assert!(
        hub.load_state(A, (true, 0), "http://localhost/form", now)
            .is_empty()
    );
    let navigations = hub.load_state(A, (false, 1), "http://localhost/form", now);
    assert_eq!(
        navigations,
        vec![
            Navigation {
                view: B,
                url: "http://localhost/form".into()
            },
            Navigation {
                view: C,
                url: "http://localhost/form".into()
            },
        ]
    );
    assert!(hub.is_loading(B, now) && hub.is_loading(C, now));
    // Followed once.
    hub.load_state(A, (true, 1), "http://localhost/form", now);
    assert!(
        hub.load_state(A, (false, 2), "http://localhost/form", now)
            .is_empty()
    );
}

#[test]
fn a_submit_that_commits_a_new_url_is_followed_once() {
    let mut hub = hub();
    let now = Instant::now();
    hub.route(A, &submit(), now);
    hub.load_state(A, (true, 0), "http://localhost/form", now);
    assert_eq!(hub.navigated(A, "http://localhost/thanks", now).len(), 2);
    assert!(
        hub.load_state(A, (false, 1), "http://localhost/thanks", now)
            .is_empty()
    );
}

/// RB-090-32: the time from the load the hub gave to the view's commit is the counter
/// `sync.navigation` (K11 for the external windows).
#[test]
fn a_given_load_that_commits_is_counted_as_a_navigation() {
    use rb_util::perf::{global, names};
    let count = || {
        global()
            .snapshot()
            .iter()
            .find(|series| series.name == names::SYNC_NAVIGATION)
            .map_or(0, |series| series.count)
    };
    let mut hub = hub();
    let now = Instant::now();
    let before = count();
    hub.mark_loading(B, now);
    hub.navigated(
        B,
        "http://localhost/next",
        now + std::time::Duration::from_millis(40),
    );
    assert!(count() > before, "the commit of a given load is counted");
    let after = count();
    // A navigation of the view's own (no load given) is not the hub's to count.
    hub.navigated(C, "http://localhost/own", now);
    assert_eq!(count(), after);
}
