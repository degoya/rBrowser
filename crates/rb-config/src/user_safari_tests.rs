use super::*;
use crate::{EngineKind, UserSettings};

#[test]
fn safari_is_on_and_takes_safari_itself_by_default() {
    let settings: UserSettings = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(
        settings.engines.safari,
        SafariSettings {
            enabled: true,
            technology_preview: false,
        }
    );
    let json = r#"{ "version": 1, "engines": { "safari": { "enabled": false, "technologyPreview": true } } }"#;
    let settings: UserSettings = serde_json::from_str(json).expect("parses");
    assert!(!settings.engines.safari.enabled);
    assert!(settings.engines.safari.technology_preview);
}

#[test]
fn safari_is_an_engine_of_its_own_in_the_config() {
    assert_eq!(EngineKind::Safari.name(), "safari");
    let parsed: EngineKind = serde_json::from_str(r#""safari""#).expect("parses");
    assert_eq!(parsed, EngineKind::Safari);
    assert_eq!(
        serde_json::to_value(EngineKind::Safari).expect("serializes"),
        serde_json::json!("safari")
    );
    // WebKit stays Playwright's build: the two never meet.
    assert_ne!(EngineKind::Safari, EngineKind::Webkit);
}
