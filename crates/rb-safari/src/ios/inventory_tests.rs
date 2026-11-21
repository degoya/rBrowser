use super::*;

/// `xcrun simctl list devices available --json`, cut down: two iOS runtimes, a watchOS one.
const SIMCTL: &str = r#"{
  "devices": {
    "com.apple.CoreSimulator.SimRuntime.iOS-17-5": [
      { "udid": "11111111-2222-3333-4444-555555555555", "name": "iPhone 16", "state": "Shutdown", "isAvailable": true },
      { "udid": "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE", "name": "iPhone SE (3rd generation)", "state": "Shutdown", "isAvailable": true }
    ],
    "com.apple.CoreSimulator.SimRuntime.iOS-18-2": [
      { "udid": "8B1E5A9C-2F3D-4C11-9E77-0A1B2C3D4E5F", "name": "iPhone 16", "state": "Booted", "isAvailable": true },
      { "udid": "99999999-8888-7777-6666-555555555555", "name": "iPad Air 11-inch (M2)", "state": "Shutdown", "isAvailable": true },
      { "udid": "12121212-3434-5656-7878-909090909090", "name": "iPhone 15", "state": "Shutdown", "isAvailable": false },
      { "udid": "--boot", "name": "iPhone 14", "state": "Shutdown", "isAvailable": true }
    ],
    "com.apple.CoreSimulator.SimRuntime.watchOS-11-2": [
      { "udid": "CCCCCCCC-DDDD-EEEE-FFFF-000000000000", "name": "Apple Watch Series 10 (46mm)", "state": "Shutdown", "isAvailable": true }
    ]
  }
}"#;

#[test]
fn simctl_lists_the_available_ios_simulators_newest_runtime_first() {
    let simulators = parse_simctl(SIMCTL);
    let rows: Vec<(&str, &str, bool)> = simulators
        .iter()
        .map(|sim| (sim.name.as_str(), sim.runtime.as_str(), sim.booted))
        .collect();
    assert_eq!(
        rows,
        [
            ("iPad Air 11-inch (M2)", "iOS 18.2", false),
            ("iPhone 16", "iOS 18.2", true),
            ("iPhone 16", "iOS 17.5", false),
            ("iPhone SE (3rd generation)", "iOS 17.5", false),
        ]
    );
    // Not available, not iOS, or no UDID: left out.
    assert!(simulators.iter().all(|sim| sim.name != "iPhone 15"));
    assert!(simulators.iter().all(|sim| sim.name != "iPhone 14"));
    assert_eq!(simulators[1].udid, "8B1E5A9C-2F3D-4C11-9E77-0A1B2C3D4E5F");
    assert_eq!(simulators[1].label(), "iPhone 16 (iOS 18.2)");
}

#[test]
fn simctl_output_that_is_no_list_gives_no_simulators() {
    assert!(parse_simctl("").is_empty());
    assert!(parse_simctl("xcrun: error: unable to find utility \"simctl\"").is_empty());
    assert!(parse_simctl(r#"{ "devices": [] }"#).is_empty());
}

#[test]
fn a_runtime_id_reads_as_its_ios_version() {
    assert_eq!(
        runtime_of("com.apple.CoreSimulator.SimRuntime.iOS-18-2"),
        Some(("iOS 18.2".to_owned(), vec![18, 2]))
    );
    assert_eq!(
        runtime_of("com.apple.CoreSimulator.SimRuntime.iOS-26-0-1"),
        Some(("iOS 26.0.1".to_owned(), vec![26, 0, 1]))
    );
    assert_eq!(
        runtime_of("com.apple.CoreSimulator.SimRuntime.watchOS-11-2"),
        None
    );
    assert_eq!(runtime_of("com.apple.CoreSimulator.SimRuntime.iOS-x"), None);
}

/// `system_profiler SPUSBDataType -json` (macOS 14): an iPhone on a hub, a keyboard.
const USB_DATA: &str = r#"{
  "SPUSBDataType": [
    { "_name": "USB31Bus", "_items": [
      { "_name": "USB2.0 Hub", "_items": [
        { "_name": "iPhone", "manufacturer": "Apple Inc.", "serial_num": "00008110001A2B3C4D5E801E", "vendor_id": "apple_vendor_id" },
        { "_name": "Magic Keyboard", "manufacturer": "Apple Inc.", "serial_num": "F0T1234567", "vendor_id": "apple_vendor_id" }
      ] }
    ] }
  ]
}"#;

/// `system_profiler SPUSBHostDataType -json` (macOS 15): an iPad, a phone of another maker.
const USB_HOST: &str = r#"{
  "SPUSBHostDataType": [
    { "_name": "USB 3.1 Bus", "_items": [
      { "_name": "iPad", "USBDeviceKeySerialNumber": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1111", "USBDeviceKeyVendorID": 1452 },
      { "_name": "iPhone Case", "USBDeviceKeySerialNumber": "00008110001A2B3C4D5E9999", "USBDeviceKeyVendorID": 4660 }
    ] }
  ]
}"#;

#[test]
fn usb_lists_the_iphones_and_ipads_with_their_udids() {
    let devices = parse_usb(USB_DATA);
    assert_eq!(
        devices,
        [Device {
            udid: "00008110-001A2B3C4D5E801E".to_owned(),
            name: "iPhone".to_owned(),
        }]
    );
    let devices = parse_usb(USB_HOST);
    assert_eq!(
        devices,
        [Device {
            udid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1111".to_owned(),
            name: "iPad".to_owned(),
        }],
        "another maker's device is no iPhone"
    );
    assert_eq!(devices[0].label(), "iPad (USB)");
}

#[test]
fn a_device_seen_twice_is_listed_once() {
    let data = USB_DATA;
    let both = format!(r#"{{ "a": {data}, "b": {data} }}"#);
    assert_eq!(parse_usb(&both).len(), 1);
    assert!(parse_usb("not json").is_empty());
}

#[test]
fn an_inventory_names_what_it_found_without_the_whole_udids() {
    let inventory = Inventory {
        xcode: true,
        simulators: parse_simctl(SIMCTL),
        devices: parse_usb(USB_DATA),
    };
    let answer = inventory.to_json();
    assert_eq!(answer["xcode"], true);
    assert_eq!(answer["simulators"][1]["name"], "iPhone 16");
    assert_eq!(answer["simulators"][1]["runtime"], "iOS 18.2");
    assert_eq!(answer["simulators"][1]["booted"], true);
    // The UDID is what `iosDevice` takes: the answer carries it, the log line does not.
    assert_eq!(answer["devices"][0]["udid"], "00008110-001A2B3C4D5E801E");
    let line = inventory.log_line();
    assert!(!line.contains("00008110-001A2B3C4D5E801E"), "{line}");
    assert!(line.contains("0000...801E"), "{line}");
}
