use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant, UNIX_EPOCH};

use super::*;

const TTL: Duration = Duration::from_secs(300);

fn ip(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, last))
}

fn pairings(limit: usize) -> Pairings {
    Pairings::new(TTL, limit)
}

#[test]
fn a_pairing_token_has_at_least_128_bits_and_hides_in_debug() {
    let mut pairings = pairings(5);
    let token = pairings.issue(Instant::now()).expect("random");
    assert!(token.expose().len() >= 32, "128 bits as hex");
    assert!(token.expose().bytes().all(|b| b.is_ascii_hexdigit()));
    let debug = format!("{token:?} {pairings:?}");
    assert!(!debug.contains(token.expose()), "{debug}");
}

#[test]
fn a_token_works_exactly_once() {
    let mut pairings = pairings(5);
    let now = Instant::now();
    let token = pairings.issue(now).expect("random");
    let (device, session) = pairings
        .redeem(token.expose(), ip(2), "Phone", now, UNIX_EPOCH)
        .expect("first redeem");
    assert_eq!(
        pairings.authenticate(session.expose(), UNIX_EPOCH),
        Some(device)
    );
    assert_eq!(
        pairings.redeem(token.expose(), ip(2), "Phone", now, UNIX_EPOCH),
        Err(PairError::Invalid)
    );
    assert_eq!(pairings.devices().len(), 1);
}

#[test]
fn an_expired_token_is_refused_and_spent() {
    let mut pairings = pairings(5);
    let now = Instant::now();
    let token = pairings.issue(now).expect("random");
    let later = now + TTL + Duration::from_secs(1);
    assert_eq!(
        pairings.redeem(token.expose(), ip(2), "Phone", later, UNIX_EPOCH),
        Err(PairError::Expired)
    );
    assert_eq!(
        pairings.redeem(token.expose(), ip(2), "Phone", now, UNIX_EPOCH),
        Err(PairError::Invalid)
    );
    assert!(pairings.devices().is_empty());
}

#[test]
fn a_new_token_replaces_the_old_one() {
    let mut pairings = pairings(5);
    let now = Instant::now();
    let old = pairings.issue(now).expect("random");
    let new = pairings.issue(now).expect("random");
    assert_ne!(old.expose(), new.expose());
    assert_eq!(
        pairings.redeem(old.expose(), ip(2), "Phone", now, UNIX_EPOCH),
        Err(PairError::Invalid)
    );
    assert!(
        pairings
            .redeem(new.expose(), ip(2), "Phone", now, UNIX_EPOCH)
            .is_ok()
    );
}

#[test]
fn five_failures_a_minute_block_the_address_but_not_others() {
    let mut pairings = pairings(5);
    let now = Instant::now();
    let token = pairings.issue(now).expect("random");
    for _ in 0..MAX_FAILURES {
        assert_eq!(
            pairings.redeem("wrong", ip(9), "Phone", now, UNIX_EPOCH),
            Err(PairError::Invalid)
        );
    }
    assert_eq!(
        pairings.redeem(token.expose(), ip(9), "Phone", now, UNIX_EPOCH),
        Err(PairError::RateLimited),
        "even the right token waits once the address is blocked"
    );
    assert!(
        pairings
            .redeem(token.expose(), ip(3), "Tablet", now, UNIX_EPOCH)
            .is_ok()
    );
    let token = pairings.issue(now).expect("random");
    let after = now + FAILURE_WINDOW + Duration::from_secs(1);
    assert!(
        pairings
            .redeem(token.expose(), ip(9), "Phone", after, UNIX_EPOCH)
            .is_ok()
    );
}

#[test]
fn the_failure_table_stays_bounded() {
    let mut limiter = RateLimiter::default();
    let now = Instant::now();
    for n in 0..(MAX_TRACKED_ADDRESSES as u32 + 50) {
        limiter.fail(IpAddr::V4(Ipv4Addr::from(n)), now);
    }
    assert!(limiter.len() <= MAX_TRACKED_ADDRESSES);
}

#[test]
fn a_full_portal_keeps_the_token_for_later() {
    let mut pairings = pairings(1);
    let now = Instant::now();
    let first = pairings.issue(now).expect("random");
    let (device, _) = pairings
        .redeem(first.expose(), ip(2), "Phone", now, UNIX_EPOCH)
        .expect("first");
    let second = pairings.issue(now).expect("random");
    assert_eq!(
        pairings.redeem(second.expose(), ip(3), "Tablet", now, UNIX_EPOCH),
        Err(PairError::Full)
    );
    assert!(pairings.disconnect(device).is_some());
    assert!(
        pairings
            .redeem(second.expose(), ip(3), "Tablet", now, UNIX_EPOCH)
            .is_ok()
    );
}

#[test]
fn disconnect_revokes_the_session_at_once() {
    let mut pairings = pairings(5);
    let now = Instant::now();
    let token = pairings.issue(now).expect("random");
    let (device, session) = pairings
        .redeem(token.expose(), ip(2), "Phone", now, UNIX_EPOCH)
        .expect("paired");
    assert!(pairings.disconnect(device).is_some());
    assert_eq!(pairings.authenticate(session.expose(), UNIX_EPOCH), None);
    assert!(
        pairings.disconnect(device).is_none(),
        "a second disconnect finds nothing"
    );
}

#[test]
fn a_device_that_never_came_back_is_unfinished_and_expires_with_the_token_time() {
    let mut pairings = pairings(5);
    let now = Instant::now();
    let token = pairings.issue(now).expect("random");
    let (quiet, quiet_session) = pairings
        .redeem(token.expose(), ip(2), "Phone", now, UNIX_EPOCH)
        .expect("paired");
    let token = pairings.issue(now).expect("random");
    let (back, session) = pairings
        .redeem(token.expose(), ip(3), "Tablet", now, UNIX_EPOCH)
        .expect("paired");
    assert_eq!(
        pairings.authenticate(session.expose(), UNIX_EPOCH),
        Some(back)
    );
    let list = pairings.devices();
    assert_eq!((list[0].id, list[0].contacted), (quiet, false));
    assert_eq!((list[1].id, list[1].contacted), (back, true));
    // Within the token time both stay.
    assert!(pairings.expire_unfinished(now + TTL / 2).is_empty());
    assert_eq!(pairings.devices().len(), 2);
    // After it the quiet one goes, with its session for the log mask.
    let gone = pairings.expire_unfinished(now + TTL);
    assert_eq!(gone.len(), 1);
    assert!(gone[0].matches(quiet_session.expose()));
    assert_eq!(
        pairings.authenticate(quiet_session.expose(), UNIX_EPOCH),
        None
    );
    let list = pairings.devices();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, back);
}

#[test]
fn an_unknown_or_empty_session_authenticates_nobody() {
    let mut pairings = pairings(5);
    assert_eq!(pairings.authenticate("", UNIX_EPOCH), None);
    assert_eq!(pairings.authenticate("abc", UNIX_EPOCH), None);
}

#[test]
fn the_device_list_carries_no_secret_and_a_bounded_name() {
    let mut pairings = pairings(5);
    let now = Instant::now();
    let token = pairings.issue(now).expect("random");
    let long_ua = "Mozilla/5.0 ".repeat(40);
    let (_, session) = pairings
        .redeem(
            token.expose(),
            ip(7),
            &long_ua,
            now,
            UNIX_EPOCH + Duration::from_secs(10),
        )
        .expect("paired");
    pairings.authenticate(session.expose(), UNIX_EPOCH + Duration::from_secs(20));
    let list = pairings.devices();
    let text = serde_json::to_string(&list).expect("serialises");
    assert!(!text.contains(session.expose()));
    assert!(!text.contains(token.expose()));
    assert_eq!(list[0].ip, "192.168.1.7");
    assert!(list[0].name.chars().count() <= MAX_NAME_CHARS);
    assert_eq!(list[0].paired_at_ms, 10_000);
    assert_eq!(list[0].last_seen_ms, 20_000);
}

#[test]
fn the_cookie_is_http_only_and_strict() {
    let mut pairings = pairings(5);
    let now = Instant::now();
    let token = pairings.issue(now).expect("random");
    let (_, session) = pairings
        .redeem(token.expose(), ip(2), "Phone", now, UNIX_EPOCH)
        .expect("paired");
    let header = set_cookie(&session);
    assert!(header.starts_with(&format!("{COOKIE_NAME}={}", session.expose())));
    assert!(header.contains("; HttpOnly"));
    assert!(header.contains("; SameSite=Strict"));
    assert!(header.contains("; Path=/"));
}

#[test]
fn the_session_is_read_from_a_cookie_header() {
    assert_eq!(session_of(Some("a=1; __rbportal=abc; b=2")), Some("abc"));
    assert_eq!(session_of(Some("a=1")), None);
    assert_eq!(session_of(None), None);
}

#[test]
fn revoking_all_hands_back_every_session() {
    let mut pairings = pairings(5);
    let mut sessions = Vec::new();
    for last in 1..=3 {
        let token = pairings.issue(Instant::now()).expect("random");
        let (_, session) = pairings
            .redeem(
                token.expose(),
                ip(last),
                "Phone",
                Instant::now(),
                SystemTime::now(),
            )
            .expect("paired");
        sessions.push(session);
    }
    let revoked = pairings.revoke_all();
    assert_eq!(revoked, sessions);
    assert!(pairings.devices().is_empty());
    assert_eq!(
        pairings.authenticate(sessions[0].expose(), SystemTime::now()),
        None
    );
}

/// RB-090-65: an asset listener's request without the cookie is a device's only from the
/// address it paired from, only once it came back with its cookie, and never after its session
/// ended.
#[test]
fn an_address_grants_a_device_only_after_its_cookie_and_until_its_session_ends() {
    let mut pairings = pairings(5);
    let now = Instant::now();
    let token = pairings.issue(now).expect("random");
    let (device, session) = pairings
        .redeem(token.expose(), ip(2), "Phone", now, UNIX_EPOCH)
        .expect("paired");
    assert_eq!(
        pairings.device_at(ip(2), UNIX_EPOCH),
        None,
        "not contacted yet"
    );
    assert_eq!(
        pairings.authenticate(session.expose(), UNIX_EPOCH),
        Some(device)
    );
    assert_eq!(pairings.device_at(ip(2), UNIX_EPOCH), Some(device));
    assert_eq!(
        pairings.device_at(ip(3), UNIX_EPOCH),
        None,
        "another address"
    );
    assert!(pairings.disconnect(device).is_some());
    assert_eq!(pairings.device_at(ip(2), UNIX_EPOCH), None, "disconnected");

    let token = pairings.issue(now).expect("random");
    let (_, session) = pairings
        .redeem(token.expose(), ip(2), "Phone", now, UNIX_EPOCH)
        .expect("paired");
    assert!(
        pairings
            .authenticate(session.expose(), UNIX_EPOCH)
            .is_some()
    );
    assert_eq!(pairings.revoke_all().len(), 1);
    assert_eq!(
        pairings.device_at(ip(2), UNIX_EPOCH),
        None,
        "the portal stopped"
    );
}
