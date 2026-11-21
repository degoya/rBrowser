use super::*;

#[test]
fn simulator_and_device_udids_are_udids() {
    assert!(is_udid("8B1E5A9C-2F3D-4C11-9E77-0A1B2C3D4E5F"));
    assert!(is_udid("00008110-001A2B3C4D5E801E"));
    assert!(is_udid("0123456789abcdef0123456789abcdef01234567"));
}

#[test]
fn names_paths_and_options_are_no_udids() {
    let long = "A".repeat(65);
    for text in [
        "",
        "iPhone 16",
        "--help",
        "-",
        "1234",
        "8B1E5A9C;rm",
        "../../etc",
        long.as_str(),
    ] {
        assert!(!is_udid(text), "{text}");
    }
}

#[test]
fn a_usb_serial_becomes_the_device_s_udid() {
    // Devices since 2018: 24 hex digits, the UDID has a dash after the eighth.
    assert_eq!(
        from_usb_serial("00008110001A2B3C4D5E801E").as_deref(),
        Some("00008110-001A2B3C4D5E801E")
    );
    // Older devices: 40 hex digits, the same as the UDID.
    let old = "0123456789abcdef0123456789abcdef01234567";
    assert_eq!(from_usb_serial(old).as_deref(), Some(old));
    // Already with its dash.
    assert_eq!(
        from_usb_serial("00008110-001A2B3C4D5E801E").as_deref(),
        Some("00008110-001A2B3C4D5E801E")
    );
    assert_eq!(from_usb_serial("C02XK0ABJG5H"), None);
    assert_eq!(from_usb_serial(""), None);
}

#[test]
fn logs_get_the_udid_cut_short() {
    assert_eq!(short("00008110-001A2B3C4D5E801E"), "0000...801E");
    assert_eq!(short("8B1E5A9C-2F3D-4C11-9E77-0A1B2C3D4E5F"), "8B1E...4E5F");
    // Too short to cut: hidden altogether.
    assert_eq!(short("ABCDEF"), "...");
}
