use std::time::Duration;

use super::*;
use crate::ios::inventory::{Device, Inventory, Simulator};

fn sim(udid: &str, name: &str, version: &[u32], booted: bool) -> Simulator {
    let text: Vec<String> = version.iter().map(u32::to_string).collect();
    Simulator {
        udid: udid.to_owned(),
        name: name.to_owned(),
        runtime: format!("iOS {}", text.join(".")),
        version: version.to_vec(),
        booted,
    }
}

const SIM_16_NEW: &str = "8B1E5A9C-2F3D-4C11-9E77-0A1B2C3D4E5F";
const SIM_16_OLD: &str = "11111111-2222-3333-4444-555555555555";
const SIM_SE: &str = "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE";
const PHONE: &str = "00008110-001A2B3C4D5E801E";

fn simulators() -> Vec<Simulator> {
    vec![
        sim(SIM_16_NEW, "iPhone 16", &[18, 2], false),
        sim(SIM_16_OLD, "iPhone 16", &[17, 5], false),
        sim(SIM_SE, "iPhone SE (3rd generation)", &[17, 5], false),
    ]
}

fn phone() -> Device {
    Device {
        udid: PHONE.to_owned(),
        name: "iPhone".to_owned(),
    }
}

fn with_xcode() -> Inventory {
    Inventory {
        xcode: true,
        simulators: simulators(),
        devices: vec![phone()],
    }
}

fn without_xcode() -> Inventory {
    Inventory {
        xcode: false,
        simulators: Vec::new(),
        devices: vec![phone()],
    }
}

fn config(mode: IosMode) -> IosConfig {
    IosConfig {
        mode,
        default_simulator: "iPhone 16".to_owned(),
        boot_timeout: Duration::from_secs(120),
    }
}

fn udid_of(target: &IosTarget) -> &str {
    target.udid()
}

#[test]
fn the_mode_reads_its_setting_and_falls_back_to_auto() {
    assert_eq!(IosMode::from_setting("auto"), IosMode::Auto);
    assert_eq!(IosMode::from_setting("simulator"), IosMode::Simulator);
    assert_eq!(IosMode::from_setting("device"), IosMode::Device);
    assert_eq!(IosMode::from_setting("bogus"), IosMode::Auto);
}

#[test]
fn with_xcode_the_simulator_comes_first_on_its_newest_runtime() {
    let target = resolve(&config(IosMode::Auto), &with_xcode(), None).expect("a target");
    assert!(target.is_simulator());
    assert_eq!(udid_of(&target), SIM_16_NEW);
    assert_eq!(target.label(), "iPhone 16 (iOS 18.2)");
}

#[test]
fn a_booted_simulator_wins_over_a_newer_runtime() {
    let mut inventory = with_xcode();
    inventory.simulators[1].booted = true;
    let target = resolve(&config(IosMode::Auto), &inventory, Some("iPhone 16")).expect("a target");
    assert_eq!(udid_of(&target), SIM_16_OLD);
}

#[test]
fn without_xcode_the_connected_device_takes_the_view() {
    let target = resolve(&config(IosMode::Auto), &without_xcode(), None).expect("a target");
    assert!(!target.is_simulator());
    assert_eq!(udid_of(&target), PHONE);
    assert_eq!(target.label(), "iPhone (USB)");
    // A breakpoint naming a simulator model runs on the device when there is no Xcode.
    let target =
        resolve(&config(IosMode::Auto), &without_xcode(), Some("iPhone 16")).expect("a target");
    assert_eq!(udid_of(&target), PHONE);
}

#[test]
fn without_xcode_and_without_a_device_both_ways_are_named() {
    let nothing = Inventory::default();
    assert_eq!(
        resolve(&config(IosMode::Auto), &nothing, None),
        Err(NoTarget::NoWay)
    );
    assert_eq!(NoTarget::NoWay.key(), "engines.ios.no.way");
    assert_eq!(NoTarget::NoWay.setting(), "engines.ios.mode");
}

#[test]
fn the_breakpoint_names_a_model_a_simulator_or_a_device() {
    let inventory = with_xcode();
    let auto = config(IosMode::Auto);
    let target = resolve(&auto, &inventory, Some(" iphone se (3rd generation) ")).expect("se");
    assert_eq!(udid_of(&target), SIM_SE);
    let target = resolve(&auto, &inventory, Some(SIM_16_OLD)).expect("by udid");
    assert_eq!(udid_of(&target), SIM_16_OLD);
    let target = resolve(&auto, &inventory, Some(&PHONE.to_lowercase())).expect("the device");
    assert!(!target.is_simulator());
    assert_eq!(
        resolve(&auto, &inventory, Some("iPhone 99")),
        Err(NoTarget::UnknownSimulator("iPhone 99".to_owned()))
    );
    assert_eq!(
        resolve(&auto, &without_xcode(), Some("00008110-0000000000000000")),
        Err(NoTarget::UnknownDevice("0000...0000".to_owned()))
    );
}

#[test]
fn the_mode_simulator_needs_xcode_and_takes_no_device() {
    let simulator = config(IosMode::Simulator);
    assert_eq!(
        resolve(&simulator, &without_xcode(), None),
        Err(NoTarget::NeedsXcode)
    );
    assert_eq!(
        resolve(&simulator, &with_xcode(), Some(PHONE)),
        Err(NoTarget::ModeExcludes("simulator"))
    );
    let target = resolve(&simulator, &with_xcode(), None).expect("a simulator");
    assert!(target.is_simulator());
}

#[test]
fn the_mode_device_takes_the_device_even_with_xcode() {
    let device = config(IosMode::Device);
    let target = resolve(&device, &with_xcode(), None).expect("the device");
    assert_eq!(udid_of(&target), PHONE);
    assert_eq!(
        resolve(&device, &with_xcode(), Some(SIM_SE)),
        Err(NoTarget::ModeExcludes("device"))
    );
    let mut none = with_xcode();
    none.devices.clear();
    assert_eq!(resolve(&device, &none, None), Err(NoTarget::NoDevice));
}

#[test]
fn xcode_without_an_ios_runtime_and_without_a_device_says_so() {
    let bare = Inventory {
        xcode: true,
        ..Inventory::default()
    };
    assert_eq!(
        resolve(&config(IosMode::Auto), &bare, None),
        Err(NoTarget::NoSimulator)
    );
}

#[test]
fn an_unknown_default_model_takes_the_newest_iphone() {
    let mut unknown = config(IosMode::Auto);
    unknown.default_simulator = "iPhone 3G".to_owned();
    let target = resolve(&unknown, &with_xcode(), None).expect("an iPhone");
    assert_eq!(udid_of(&target), SIM_16_NEW);
}

#[test]
fn the_session_asks_for_mobile_safari_on_the_one_udid() {
    let simulator = resolve(&config(IosMode::Auto), &with_xcode(), None).expect("a simulator");
    let caps = capabilities(&simulator);
    assert_eq!(caps["browserName"], "safari");
    assert_eq!(caps["platformName"], "iOS");
    assert_eq!(caps["safari:useSimulator"], true);
    assert_eq!(caps["safari:deviceUDID"], SIM_16_NEW);
    let device = resolve(&config(IosMode::Device), &with_xcode(), None).expect("the device");
    let caps = capabilities(&device);
    assert_eq!(caps["safari:useSimulator"], false);
    assert_eq!(caps["safari:deviceUDID"], PHONE);
}

#[test]
fn every_refusal_has_a_text_key_and_the_mode_setting() {
    for refusal in [
        NoTarget::NoWay,
        NoTarget::NeedsXcode,
        NoTarget::NoSimulator,
        NoTarget::NoDevice,
        NoTarget::UnknownSimulator("iPhone 99".to_owned()),
        NoTarget::UnknownDevice("0000...0000".to_owned()),
        NoTarget::ModeExcludes("device"),
    ] {
        assert!(refusal.key().starts_with("engines.ios.no."), "{refusal:?}");
        assert_eq!(refusal.setting(), "engines.ios.mode");
        assert!(!refusal.reason().is_empty());
    }
}
