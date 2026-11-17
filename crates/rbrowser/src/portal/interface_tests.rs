use std::net::Ipv4Addr;

use super::*;

#[test]
fn an_explicit_address_is_taken() {
    assert_eq!(
        listen_address("192.168.1.20"),
        Ok(Ipv4Addr::new(192, 168, 1, 20))
    );
    assert_eq!(listen_address(" 127.0.0.1 "), Ok(Ipv4Addr::LOCALHOST));
}

#[test]
fn every_interface_is_never_taken() {
    assert_eq!(listen_address("0.0.0.0"), Err(InterfaceError::Unspecified));
    assert_eq!(
        listen_address("255.255.255.255"),
        Err(InterfaceError::Invalid)
    );
    assert_eq!(listen_address("224.0.0.1"), Err(InterfaceError::Invalid));
    assert_eq!(listen_address("eth0"), Err(InterfaceError::Invalid));
    assert_eq!(listen_address("::1"), Err(InterfaceError::Invalid));
}

#[test]
fn the_default_route_is_never_unspecified() {
    // Without a network the default route may be missing; with one it is a private address.
    if let Ok(address) = listen_address("") {
        assert!(!address.is_unspecified());
        assert!(is_private(address), "{address}");
    }
}

#[test]
fn without_a_setting_only_a_private_address_is_taken() {
    for private in [
        "10.0.0.7",
        "172.16.0.1",
        "172.31.255.254",
        "192.168.1.20",
        "169.254.10.1",
        "100.64.0.1",
        "100.127.255.254",
    ] {
        let address: Ipv4Addr = private.parse().expect("an address");
        assert_eq!(default_address(Some(address)), Ok(address), "{private}");
    }
    for public in [
        "8.8.8.8",
        "172.32.0.1",
        "100.128.0.1",
        "193.168.1.1",
        "127.0.0.1",
    ] {
        let address: Ipv4Addr = public.parse().expect("an address");
        assert_eq!(
            default_address(Some(address)),
            Err(InterfaceError::NotPrivate(address)),
            "{public}"
        );
    }
    assert_eq!(default_address(None), Err(InterfaceError::NoLan));
    assert_eq!(
        default_address(Some(Ipv4Addr::UNSPECIFIED)),
        Err(InterfaceError::NoLan)
    );
}

#[test]
fn an_explicit_public_address_is_honoured() {
    assert_eq!(listen_address("8.8.8.8"), Ok(Ipv4Addr::new(8, 8, 8, 8)));
}

#[test]
fn a_refused_default_route_says_to_set_the_interface() {
    let error = InterfaceError::NotPrivate(Ipv4Addr::new(203, 0, 113, 9));
    let text = error.to_string();
    assert!(
        text.contains("203.0.113.9") && text.contains("portal.interface"),
        "{text}"
    );
    assert_eq!(error.message_key(), "portal.notice.not_private");
}
