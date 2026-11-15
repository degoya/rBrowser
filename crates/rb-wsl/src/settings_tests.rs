use serde_json::json;

use super::*;

#[test]
fn defaults_without_a_section() {
    let settings = WslSettings::from_value(None);
    assert_eq!(settings, WslSettings::default());
    assert_eq!(settings.networking_mode_override.mode(), None);
    assert_eq!(settings.helper_path(), None);
}

#[test]
fn a_full_section() {
    let value = json!({
        "helperPath": "/opt/rbrowser-wsl",
        "distro": "Debian",
        "networkingModeOverride": "mirrored"
    });
    let settings = WslSettings::from_value(Some(&value));
    assert_eq!(settings.helper_path(), Some("/opt/rbrowser-wsl"));
    assert_eq!(settings.distro(), Some("Debian"));
    assert_eq!(
        settings.networking_mode_override.mode(),
        Some(NetworkingMode::Mirrored)
    );
    assert_eq!(ModeOverride::Nat.mode(), Some(NetworkingMode::Nat));
}

#[test]
fn blank_values_and_a_section_that_does_not_read() {
    let settings = WslSettings::from_value(Some(&json!({ "distro": "  ", "helperPath": "" })));
    assert_eq!(settings.distro(), None);
    assert_eq!(settings.helper_path(), None);
    let wrong = WslSettings::from_value(Some(&json!({ "networkingModeOverride": "bridged" })));
    assert_eq!(wrong, WslSettings::default());
}
