use std::time::Duration;

use super::*;

#[test]
fn the_timeout_defaults_to_thirty_seconds() {
    assert_eq!(
        SidecarSettings::default().rpc_timeout(),
        Duration::from_secs(30)
    );
}

#[test]
fn the_timeout_is_clamped_to_its_range() {
    let short = SidecarSettings {
        rpc_timeout_ms: 5,
        ..SidecarSettings::default()
    };
    let long = SidecarSettings {
        rpc_timeout_ms: u32::MAX,
        ..SidecarSettings::default()
    };
    assert_eq!(short.rpc_timeout(), Duration::from_secs(1));
    assert_eq!(long.rpc_timeout(), Duration::from_secs(600));
}

#[test]
fn a_blank_path_counts_as_none() {
    assert_eq!(non_empty(Some("  ")), None);
    assert_eq!(non_empty(None), None);
    assert_eq!(non_empty(Some(" C:/node.exe ")), Some("C:/node.exe"));
}
