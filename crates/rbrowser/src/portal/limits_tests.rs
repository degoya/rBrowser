use std::net::Ipv4Addr;

use super::*;

fn ip(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, last))
}

#[test]
fn one_address_holds_at_most_its_share() {
    let mut limits = Limits::default();
    for _ in 0..MAX_PER_ADDRESS {
        assert!(limits.acquire(ip(1)));
    }
    assert!(!limits.acquire(ip(1)), "one over the share");
    assert!(limits.acquire(ip(2)), "another address still gets in");
    limits.release(ip(1));
    assert!(limits.acquire(ip(1)), "a released slot comes back");
}

#[test]
fn the_total_is_capped_across_addresses() {
    let mut limits = Limits::default();
    let mut last = 0u8;
    while limits.total() < MAX_CONNECTIONS {
        last += 1;
        for _ in 0..MAX_PER_ADDRESS.min(MAX_CONNECTIONS - limits.total()) {
            assert!(limits.acquire(ip(last)));
        }
    }
    assert!(!limits.acquire(ip(200)), "the portal is full");
}

#[test]
fn releasing_forgets_an_idle_address_and_never_underflows() {
    let mut limits = Limits::default();
    assert!(limits.acquire(ip(1)));
    limits.release(ip(1));
    limits.release(ip(1));
    limits.release(ip(9));
    assert_eq!(limits.total(), 0);
    assert_eq!(limits.addresses(), 0);
}
