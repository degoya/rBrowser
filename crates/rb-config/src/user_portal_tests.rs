use serde_json::json;

use super::*;
use crate::UserSettings;

#[test]
fn the_portal_is_off_by_default_with_safe_limits() {
    let settings = UserSettings::default().portal;
    assert!(!settings.enabled);
    assert_eq!(settings.interface, "");
    assert_eq!(settings.port, None);
    assert_eq!(settings.pairing_ttl_sec, 300);
    assert_eq!(settings.max_devices, 5);
}

#[test]
fn the_keys_are_camel_case_and_a_missing_one_defaults() {
    let settings: UserSettings = serde_json::from_value(json!({
        "portal": { "enabled": true, "interface": "192.168.1.20", "pairingTtlSec": 60 }
    }))
    .expect("parses");
    assert!(settings.portal.enabled);
    assert_eq!(settings.portal.interface, "192.168.1.20");
    assert_eq!(settings.portal.pairing_ttl_sec, 60);
    assert_eq!(settings.portal.max_devices, 5);
    let written = serde_json::to_value(&settings.portal).expect("serialises");
    assert_eq!(written["maxDevices"], 5);
    assert!(
        written.get("port").is_none(),
        "no port is written when none is set"
    );
}

#[test]
fn values_out_of_range_are_clamped_when_used() {
    let settings = PortalSettings {
        pairing_ttl_sec: 1,
        max_devices: 0,
        ..PortalSettings::default()
    };
    assert_eq!(
        settings.pairing_ttl(),
        std::time::Duration::from_secs(MIN_PAIRING_TTL_SEC.into())
    );
    assert_eq!(settings.device_limit(), 1);
    let settings = PortalSettings {
        pairing_ttl_sec: 1_000_000,
        max_devices: 1000,
        ..PortalSettings::default()
    };
    assert_eq!(
        settings.pairing_ttl(),
        std::time::Duration::from_secs(MAX_PAIRING_TTL_SEC.into())
    );
    assert_eq!(settings.device_limit(), MAX_DEVICES as usize);
}
