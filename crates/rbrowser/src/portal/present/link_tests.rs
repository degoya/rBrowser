use std::time::{Duration, Instant};

use super::*;

const TTL: Duration = Duration::from_secs(600);

#[test]
fn a_link_is_128_bits_and_works_until_it_is_spent() {
    let mut links = ViewerLinks::new(TTL);
    let now = Instant::now();
    assert_eq!(links.check("", now), Err(PairError::Invalid), "none issued");
    let token = links.issue(now).expect("random");
    assert_eq!(token.expose().len(), LINK_BYTES * 2);
    assert_eq!(links.check(token.expose(), now), Ok(()));
    assert!(links.is_open(now));
    links.spend();
    assert!(!links.is_open(now));
    assert_eq!(
        links.check(token.expose(), now),
        Err(PairError::Invalid),
        "a spent link works no more"
    );
}

#[test]
fn a_new_link_voids_the_one_before() {
    let mut links = ViewerLinks::new(TTL);
    let now = Instant::now();
    let old = links.issue(now).expect("random");
    let new = links.issue(now).expect("random");
    assert_ne!(old, new);
    assert_eq!(links.check(old.expose(), now), Err(PairError::Invalid));
    assert_eq!(links.check(new.expose(), now), Ok(()));
}

#[test]
fn an_expired_link_is_spent() {
    let mut links = ViewerLinks::new(TTL);
    let now = Instant::now();
    let token = links.issue(now).expect("random");
    assert_eq!(links.expires(), Some(now + TTL));
    assert_eq!(
        links.check(token.expose(), now + TTL),
        Err(PairError::Expired)
    );
    assert_eq!(
        links.check(token.expose(), now),
        Err(PairError::Invalid),
        "the expiry spent it"
    );
}

/// Behind the tunnel every request comes from 127.0.0.1, so failures count per link: the
/// fifth wrong token within a minute spends the link, even for the right one after it.
#[test]
fn five_failures_within_a_minute_spend_the_link() {
    let mut links = ViewerLinks::new(TTL);
    let now = Instant::now();
    let token = links.issue(now).expect("random");
    for attempt in 0..MAX_LINK_FAILURES {
        assert_eq!(
            links.check("0123456789abcdef0123456789abcdef", now),
            Err(PairError::Invalid),
            "attempt {attempt}"
        );
    }
    assert!(!links.is_open(now));
    assert_eq!(links.check(token.expose(), now), Err(PairError::Invalid));
    // A person's new link starts with no failures of its own.
    let fresh = links.issue(now).expect("random");
    assert_eq!(links.check(fresh.expose(), now), Ok(()));
}

#[test]
fn failures_older_than_a_minute_are_forgotten() {
    let mut links = ViewerLinks::new(TTL);
    let start = Instant::now();
    let token = links.issue(start).expect("random");
    for _ in 0..MAX_LINK_FAILURES - 1 {
        let _ = links.check("wrong", start);
    }
    let later = start + FAILURE_WINDOW + Duration::from_secs(1);
    assert_eq!(links.check("wrong", later), Err(PairError::Invalid));
    assert!(links.is_open(later), "one failure in the window");
    assert_eq!(links.check(token.expose(), later), Ok(()));
}

/// Across links the failures count too: past [`MAX_GLOBAL_FAILURES`] within a minute every
/// attempt is refused, the right token included, until the window passed.
#[test]
fn many_failures_across_links_lock_every_attempt() {
    let mut links = ViewerLinks::new(TTL);
    let now = Instant::now();
    let mut failures = 0;
    while failures < MAX_GLOBAL_FAILURES {
        links.issue(now).expect("random");
        for _ in 0..MAX_LINK_FAILURES.min(MAX_GLOBAL_FAILURES - failures) {
            let _ = links.check("wrong", now);
            failures += 1;
        }
    }
    let token = links.issue(now).expect("random");
    assert_eq!(
        links.check(token.expose(), now),
        Err(PairError::RateLimited)
    );
    let later = now + FAILURE_WINDOW + Duration::from_secs(1);
    assert_eq!(links.check(token.expose(), later), Ok(()));
}

#[test]
fn every_token_issued_is_handed_back_for_the_log_mask() {
    let mut links = ViewerLinks::new(TTL);
    let now = Instant::now();
    let first = links.issue(now).expect("random");
    let second = links.issue(now).expect("random");
    let issued = links.revoke_all();
    assert_eq!(issued, [first, second]);
    assert!(!links.is_open(now));
    assert!(links.revoke_all().is_empty());
}

#[test]
fn the_debug_output_shows_no_token() {
    let mut links = ViewerLinks::new(TTL);
    let token = links.issue(Instant::now()).expect("random");
    let debug = format!("{links:?}");
    assert!(!debug.contains(token.expose()), "{debug}");
}

#[test]
fn the_link_carries_the_token_in_the_fragment_only() {
    let link = link_url("https://quick-brown-fox.trycloudflare.com", "00ff");
    assert_eq!(
        link,
        "https://quick-brown-fox.trycloudflare.com/__rbrowser/join#t=00ff"
    );
    let (before, _) = link.split_once('#').expect("a fragment");
    assert!(!before.contains("00ff"), "no token in what is sent");
}
