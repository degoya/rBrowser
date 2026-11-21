use super::*;

const LISTING: &str = "\
* daemon not running; starting now at tcp:5037
* daemon started successfully
List of devices attached
R58N123ABC             device usb:1-1 product:o1sxeea model:SM_G991B device:o1s transport_id:1
emulator-5554          offline transport_id:2
0A1B2C3D               unauthorized usb:1-2 transport_id:3
ZY22                   no permissions (user in plugdev group; are your udev rules wrong?); see [http://developer.android.com/tools/device.html] usb:1-3

";

#[test]
fn the_listing_reads_every_device_with_its_state_and_model() {
    let devices = parse_devices(LISTING);
    assert_eq!(devices.len(), 4, "{devices:?}");
    assert_eq!(
        devices[0],
        Device {
            serial: "R58N123ABC".to_owned(),
            state: DeviceState::Device,
            model: Some("SM G991B".to_owned()),
            product: Some("o1sxeea".to_owned()),
        }
    );
    assert_eq!(devices[1].state, DeviceState::Offline);
    assert_eq!(devices[1].model, None);
    assert_eq!(devices[2].state, DeviceState::Unauthorized);
    assert_eq!(devices[3].state, DeviceState::NoPermissions);
    assert!(devices[0].is_ready());
    assert!(!devices[2].is_ready());
}

/// A serial is passed to adb after `-s`: one that could be read as an option, or that names a
/// device over the network (`host:port`, not in this job), is never taken.
#[test]
fn only_plain_usb_serials_are_taken() {
    for serial in ["R58N123ABC", "emulator-5554", "0123456789abcdef", "a_b.c"] {
        assert!(valid_serial(serial), "{serial}");
    }
    for serial in [
        "",
        "-s",
        "--help",
        ".hidden",
        "192.168.1.5:5555",
        "a b",
        "x;rm",
        "adb-XYZ._adb-tls-connect._tcp:1",
    ] {
        assert!(!valid_serial(serial), "{serial:?}");
    }
    assert!(!valid_serial(&"a".repeat(MAX_SERIAL + 1)));
    let listing = "List of devices attached\n-evil device\n192.168.1.5:5555 device\nOK1 device\n";
    let serials: Vec<String> = parse_devices(listing)
        .into_iter()
        .map(|device| device.serial)
        .collect();
    assert_eq!(serials, ["OK1"]);
}

#[test]
fn a_breakpoint_id_names_the_serial_and_finds_its_device_again() {
    let devices = parse_devices(LISTING);
    assert_eq!(breakpoint_id("R58N123ABC"), "android-R58N123ABC");
    let long = "X".repeat(MAX_SERIAL);
    let id = breakpoint_id(&long);
    assert!(id.chars().count() <= 40, "{id}");
    assert_eq!(
        serial_for("android-R58N123ABC", &devices).map(|d| d.serial.as_str()),
        Some("R58N123ABC")
    );
    assert!(serial_for("android-unknown", &devices).is_none());
    assert!(serial_for("mobile", &devices).is_none());
}

/// Two long serials that share their start get two ids: the cut serial carries a hash of the
/// whole one, and each id finds its own device.
#[test]
fn long_serials_that_share_their_start_keep_their_own_ids() {
    let first = format!("{}A", "R".repeat(MAX_SERIAL - 1));
    let second = format!("{}B", "R".repeat(MAX_SERIAL - 1));
    let (one, two) = (breakpoint_id(&first), breakpoint_id(&second));
    assert_ne!(one, two);
    for id in [&one, &two] {
        assert!(id.len() <= 40, "{id}");
        assert!(id.starts_with("android-RRRR"), "{id}");
    }
    assert_eq!(one, breakpoint_id(&first), "stable");
    let listing = format!("List of devices attached\n{first} device\n{second} device\n");
    let devices = parse_devices(&listing);
    assert_eq!(
        serial_for(&two, &devices).map(|d| d.serial.as_str()),
        Some(second.as_str())
    );
    // A serial that fits keeps the plain id.
    let fits = "S".repeat(40 - "android-".len());
    assert_eq!(breakpoint_id(&fits), format!("android-{fits}"));
}

#[test]
fn the_screen_is_read_in_css_px_with_its_pixel_ratio() {
    let size = "Physical size: 1080x2400\n";
    let density = "Physical density: 420\n";
    let screen = parse_screen(size, density).expect("a screen");
    assert_eq!(screen.width_px, 1080);
    assert_eq!(screen.height_px, 2400);
    assert!((screen.dpr - 2.625).abs() < 1e-9);
    assert_eq!(screen.css_size(), (411, 914));
    // An override (`wm size 720x1600`) is what the page sees.
    let screen = parse_screen(
        "Physical size: 1080x2400\nOverride size: 720x1600\n",
        "Physical density: 420\nOverride density: 320\n",
    )
    .expect("a screen");
    assert_eq!((screen.width_px, screen.height_px), (720, 1600));
    assert!((screen.dpr - 2.0).abs() < 1e-9);
    assert_eq!(screen.css_size(), (360, 800));
    assert!(parse_screen("error: no devices", density).is_none());
    assert!(parse_screen(size, "Physical density: 0").is_none());
}

#[test]
fn the_label_names_the_model_and_the_device_size() {
    let devices = parse_devices(LISTING);
    assert_eq!(devices[0].name(), "SM G991B");
    assert_eq!(devices[1].name(), "emulator-5554");
}
