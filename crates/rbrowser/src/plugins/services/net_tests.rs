use std::net::IpAddr;

use super::{Origin, is_public};

fn ip(text: &str) -> IpAddr {
    text.parse().expect("an address")
}

#[test]
fn only_public_addresses_are_public() {
    for private in [
        "0.0.0.0",
        "0.1.2.3",
        "10.0.0.1",
        "100.64.0.1",
        "127.0.0.1",
        "127.255.255.254",
        "169.254.169.254",
        "172.16.0.1",
        "172.31.255.255",
        "192.0.0.8",
        "192.0.2.1",
        "192.168.1.1",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "240.0.0.1",
        "255.255.255.255",
        "::",
        "::1",
        "fc00::1",
        "fd12:3456::1",
        "fe80::1",
        "fec0::1",
        "ff02::1",
        "2001:db8::1",
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.1",
        "64:ff9b::a9fe:a9fe",
        "::127.0.0.1",
    ] {
        assert!(!is_public(ip(private)), "{private}");
    }
    for public in [
        "1.1.1.1",
        "8.8.8.8",
        "172.15.255.255",
        "172.32.0.1",
        "100.63.255.255",
        "100.128.0.1",
        "93.184.216.34",
        "2606:4700:4700::1111",
        "::ffff:8.8.8.8",
        "64:ff9b::808:808",
    ] {
        assert!(is_public(ip(public)), "{public}");
    }
}

#[test]
fn an_origin_is_scheme_host_and_port_with_the_default_ports_filled_in() {
    let origin = Origin::of("http://LocalHost:3000/path?q#f").expect("origin");
    assert_eq!(origin, Origin::of("http://localhost:3000").expect("origin"));
    assert_ne!(
        origin,
        Origin::of("https://localhost:3000").expect("origin")
    );
    assert_ne!(origin, Origin::of("http://localhost:3001").expect("origin"));
    assert_eq!(
        Origin::of("https://site.test/").expect("origin"),
        Origin::of("https://site.test:443/x").expect("origin")
    );
    assert_eq!(
        Origin::of("http://[::1]:8080/").expect("origin"),
        Origin::of("http://[::1]:8080").expect("origin")
    );
    for refused in ["file:///etc/passwd", "localhost:3000", "", "http://"] {
        assert!(Origin::of(refused).is_none(), "{refused}");
    }
}

#[test]
fn the_ipv6_ranges_that_reach_private_or_special_hosts_are_not_public() {
    // Review finding L3 of Charge 6.
    for special in [
        // IPv4-translated (RFC 2765): the IPv4 address inside decides.
        "::ffff:0:127.0.0.1",
        "::ffff:0:a9fe:a9fe",
        // Local-use NAT64 (RFC 8215).
        "64:ff9b:1::808:808",
        "64:ff9b:1:ffff::1",
        // 6to4 (RFC 3056, deprecated by RFC 7526).
        "2002:7f00:1::",
        "2002:808:808::1",
        // Teredo and the rest of the IETF protocol block 2001::/23 (benchmarking, ORCHID).
        "2001::1",
        "2001:0:4136:e378:8000:63bf:3fff:fdd2",
        "2001:2::1",
        "2001:10::1",
        "2001:20::1",
        "2001:1ff:ffff::1",
        // Discard-only (RFC 6666).
        "100::1",
        "100::ffff:ffff:ffff:ffff",
        // Documentation (RFC 9637) and SRv6 SIDs (RFC 9602).
        "3fff::1",
        "3fff:fff::1",
        "5f00::1",
    ] {
        assert!(!is_public(ip(special)), "{special}");
    }
    for public in [
        "::ffff:0:808:808",
        "2001:200::1",
        "2001:4860:4860::8888",
        "100:0:0:1::1",
        "3fff:1000::1",
        "2003::1",
    ] {
        assert!(is_public(ip(public)), "{public}");
    }
}
