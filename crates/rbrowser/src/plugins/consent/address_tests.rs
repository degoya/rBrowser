use std::net::IpAddr;

use super::{Admission, host_port};
use crate::plugins::services::net::Origin;

fn ip(text: &str) -> IpAddr {
    text.parse().expect("ip")
}

fn held(own: &[&str]) -> Admission {
    Admission {
        any: false,
        own: own.iter().filter_map(|url| Origin::of(url)).collect(),
    }
}

#[test]
fn a_public_address_is_probed() {
    let admission = held(&[]);
    assert!(admission.allows("https://example.test/", &[ip("93.184.216.34")]));
    assert!(admission.allows("https://example.test/", &[ip("2606:2800:220:1::1")]));
}

#[test]
fn a_local_or_lan_address_is_not() {
    let admission = held(&[]);
    for address in [
        "127.0.0.1",
        "10.0.0.5",
        "192.168.1.1",
        "172.16.0.1",
        "169.254.169.254",
        "::1",
        "fd00::1",
        "::ffff:10.0.0.1",
    ] {
        assert!(
            !admission.allows("http://intranet.test/", &[ip(address)]),
            "{address}"
        );
    }
}

#[test]
fn every_address_of_a_name_must_be_public() {
    // The browser picks any of them: one private address is enough to refuse.
    let admission = held(&[]);
    assert!(!admission.allows(
        "https://example.test/",
        &[ip("93.184.216.34"), ip("10.0.0.5")]
    ));
    // A name that did not resolve is no public address.
    assert!(!admission.allows("https://example.test/", &[]));
}

#[test]
fn the_trusted_projects_own_sites_are_probed_wherever_they_are() {
    let admission = held(&["http://localhost:3000", "https://staging.example.test"]);
    assert!(admission.allows("http://localhost:3000/shop", &[ip("127.0.0.1")]));
    assert!(admission.allows("https://staging.example.test/", &[ip("10.0.0.5")]));
    // Another port or scheme is another origin.
    assert!(!admission.allows("http://localhost:3001/", &[ip("127.0.0.1")]));
    assert!(!admission.allows("https://localhost:3000/", &[ip("127.0.0.1")]));
}

#[test]
fn the_persons_setting_allows_any_address() {
    let admission = Admission {
        any: true,
        own: Vec::new(),
    };
    assert!(admission.allows("http://intranet.test/", &[ip("10.0.0.5")]));
    assert!(admission.allows("http://intranet.test/", &[]));
}

#[test]
fn only_http_and_https_pages_are_held_to_the_rule() {
    let admission = held(&[]);
    assert!(!admission.allows("about:blank", &[ip("93.184.216.34")]));
    assert!(!admission.allows("file:///C:/secret.txt", &[ip("93.184.216.34")]));
    assert!(!admission.allows("data:text/html,x", &[ip("93.184.216.34")]));
}

#[test]
fn the_host_and_port_are_read_as_the_browser_reads_them() {
    assert_eq!(
        host_port("https://Example.TEST/a"),
        Some(("example.test".to_owned(), 443))
    );
    assert_eq!(
        host_port("http://example.test:8080/"),
        Some(("example.test".to_owned(), 8080))
    );
    assert_eq!(
        host_port("http://[::1]:3000/"),
        Some(("::1".to_owned(), 3000))
    );
    assert_eq!(
        host_port("http://127.1/"),
        Some(("127.0.0.1".to_owned(), 80))
    );
    assert_eq!(host_port("about:blank"), None);
}

#[tokio::test]
async fn an_address_literal_is_checked_without_a_lookup() {
    let admission = held(&[]);
    assert!(admission.check("http://127.0.0.1/").await.is_err());
    assert!(admission.check("http://[::1]:8080/").await.is_err());
    // WHATWG reads `127.1` and `0x7f.1` as 127.0.0.1.
    assert!(admission.check("http://127.1/").await.is_err());
    assert!(admission.check("http://0x7f.1/").await.is_err());
    assert!(admission.check("http://1.1.1.1/").await.is_ok());
    assert!(admission.check("about:blank").await.is_err());
}
