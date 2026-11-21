use serde_json::json;

use super::*;

/// A user agent value the masker must hide (a test value, built so no scanner takes it for a key).
const UA_SECRET: &str = "ua-secret-4715";

fn masker() -> Masker {
    Masker::new(["header-value-4714"])
}

#[test]
fn the_emulation_names_what_is_set_and_never_a_header_value() {
    let emulation = json!({
        "width": 390, "height": 844, "dpr": 3.0, "mobile": true, "touch": true,
        "userAgent": format!("Mozilla/5.0 (iPhone) token={UA_SECRET}"),
        "locale": "de-DE", "colorScheme": "dark",
        "media": { "features": [] },
        "headers": { "X-Api-Key": "header-value-4714", "Authorization": "Bearer header-value-4714" },
        "geolocation": { "latitude": 52.5, "longitude": 13.4 },
        "timezone": "Europe/Berlin", "cpuThrottling": 4.0
    });
    let lines = emulation_lines(&emulation, &masker());
    let shown = format!("{lines:?}");
    for secret in ["header-value-4714", UA_SECRET, "52.5", "13.4"] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
    let get = |name: &str| {
        lines
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(get("mobile"), Some("true"));
    assert_eq!(get("locale"), Some("de-DE"));
    assert_eq!(get("colorScheme"), Some("dark"));
    assert_eq!(get("headers"), Some("Authorization, X-Api-Key"));
    assert_eq!(get("geolocation"), Some("set"));
    assert_eq!(get("cpuThrottling"), Some("4.0"));
    assert_eq!(get("width"), None, "the size is the view's own line");
    assert_eq!(get("media"), Some("{\"features\":[]}"));
}

#[test]
fn an_emulation_that_is_no_object_names_nothing() {
    assert!(emulation_lines(&json!(null), &masker()).is_empty());
    assert!(emulation_lines(&json!([1, 2]), &masker()).is_empty());
}

#[test]
fn header_values_are_secrets_of_the_report() {
    let emulation = json!({ "headers": { "X-Api-Key": "header-value-4714", "X-Short": "ab" } });
    assert_eq!(header_values(&emulation), ["header-value-4714", "ab"]);
    assert!(header_values(&json!({})).is_empty());
}
