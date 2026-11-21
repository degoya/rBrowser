use serde_json::json;

use super::*;
use crate::UserSettings;
use crate::annotate::USER_KEYS;

#[test]
fn the_presentation_is_off_and_not_agreed_by_default() {
    let settings = UserSettings::default().portal.presentation;
    assert!(!settings.enabled);
    assert!(!settings.consented);
    assert_eq!(settings.link_ttl_sec, 600);
    assert_eq!(settings.max_viewers, 3);
    assert!(settings.allow_paths.is_empty(), "the start page and below");
    for denied in [
        "/admin",
        "/.env",
        "/.git",
        "/_debug",
        "/@fs/",
        "/__open-in-editor",
        "/__open-stack-frame-in-editor",
        "/__nextjs_launch-editor",
        "/__nextjs_original-stack-frame",
    ] {
        assert!(
            settings.deny_paths.iter().any(|path| path == denied),
            "{denied} is denied by default"
        );
    }
    assert!(settings.pointer);
}

#[test]
fn the_keys_are_camel_case_below_portal() {
    let settings: UserSettings = serde_json::from_value(json!({
        "portal": { "presentation": {
            "enabled": true, "linkTtlSec": 120, "maxViewers": 1,
            "allowPaths": ["/shop"], "denyPaths": [], "pointer": false
        } }
    }))
    .expect("parses");
    let presentation = &settings.portal.presentation;
    assert!(presentation.enabled);
    assert!(!presentation.consented, "a missing key defaults");
    assert_eq!(presentation.link_ttl_sec, 120);
    assert_eq!(presentation.max_viewers, 1);
    assert_eq!(presentation.allow_paths, ["/shop"]);
    assert!(presentation.deny_paths.is_empty(), "a person may clear it");
    assert!(!presentation.pointer);
    let written = serde_json::to_value(&settings.portal).expect("serialises");
    assert_eq!(written["presentation"]["linkTtlSec"], 120);
}

#[test]
fn values_out_of_range_are_clamped_when_used() {
    let low = PresentationSettings {
        link_ttl_sec: 1,
        max_viewers: 0,
        ..PresentationSettings::default()
    };
    assert_eq!(low.link_ttl(), Duration::from_secs(MIN_LINK_TTL_SEC.into()));
    assert_eq!(low.viewer_limit(), 1);
    let high = PresentationSettings {
        link_ttl_sec: u32::MAX,
        max_viewers: u32::MAX,
        ..PresentationSettings::default()
    };
    assert_eq!(
        high.link_ttl(),
        Duration::from_secs(MAX_LINK_TTL_SEC.into())
    );
    assert_eq!(high.viewer_limit(), MAX_VIEWERS as usize);
}

/// The consent is no setting of a running presentation: agreeing stops nothing, every other
/// key does.
#[test]
fn the_session_keys_leave_the_consent_out() {
    let before = PresentationSettings::default();
    let agreed = PresentationSettings {
        consented: true,
        ..before.clone()
    };
    assert_eq!(before.session(), agreed.session());
    let fewer = PresentationSettings {
        max_viewers: 1,
        ..before.clone()
    };
    assert_ne!(before.session(), fewer.session());
}

/// A change of the presentation's keys never stops the LAN portal.
#[test]
fn the_lan_listener_ignores_the_presentation() {
    let portal = crate::PortalSettings::default();
    let changed = crate::PortalSettings {
        presentation: PresentationSettings {
            enabled: true,
            ..PresentationSettings::default()
        },
        ..portal.clone()
    };
    assert_eq!(portal.listener(), changed.listener());
}

/// Each key widens what reaches the internet, so none is MCP's to set; all sit in Security &
/// Privacy > Portal and pairing, the program path beside them.
#[test]
fn every_presentation_key_is_protected_and_in_the_portal_section() {
    for key in [
        "portal.presentation.enabled",
        "portal.presentation.consented",
        "portal.presentation.linkTtlSec",
        "portal.presentation.maxViewers",
        "portal.presentation.allowPaths",
        "portal.presentation.denyPaths",
        "portal.presentation.pointer",
        "tools.cloudflaredPath",
    ] {
        let def = USER_KEYS
            .iter()
            .find(|def| def.key == key)
            .unwrap_or_else(|| panic!("{key} is a key"));
        assert!(def.protected, "{key} is open to MCP");
        assert_eq!(def.category, rb_settings::Category::Security, "{key}");
        assert_eq!(def.section, "portal", "{key}");
    }
}

#[test]
fn the_cloudflared_path_is_a_user_setting_only() {
    let user: UserSettings = serde_json::from_value(json!({
        "tools": { "cloudflaredPath": "C:\\Tools\\cloudflared.exe" }
    }))
    .expect("parses");
    assert_eq!(
        user.tools.cloudflared_path.as_deref(),
        Some("C:\\Tools\\cloudflared.exe")
    );
}
