use serde_json::json;

use super::*;
use crate::vars::substitute;

fn layers() -> Layers {
    Layers {
        public: EnvFile::from_json(&json!({
            "$shared": { "version": "v1", "baseUrl": "https://shared.example" },
            "staging": { "baseUrl": "https://staging.example", "user": "demo" }
        }))
        .expect("reads"),
        private: EnvFile::from_json(&json!({
            "$shared": { "token": "shared-token" },
            "staging": { "token": "staging-token" }
        }))
        .expect("reads"),
        environment: Some("staging".into()),
        base_url: Some("https://from-rbrowser.example".into()),
    }
}

#[test]
fn the_environment_wins_over_shared_and_private_over_public() {
    let layers = layers();
    assert_eq!(
        layers.variable("token"),
        Some(Value::secret("staging-token"))
    );
    assert_eq!(
        layers.variable("baseUrl"),
        Some(Value::plain("https://staging.example"))
    );
    assert_eq!(layers.variable("version"), Some(Value::plain("v1")));
    assert_eq!(layers.variable("nothing"), None);
}

#[test]
fn shared_values_hold_without_an_environment() {
    let mut layers = layers();
    layers.environment = None;
    assert_eq!(
        layers.variable("token"),
        Some(Value::secret("shared-token"))
    );
    assert_eq!(
        layers.variable("baseUrl"),
        Some(Value::plain("https://shared.example"))
    );
}

#[test]
fn the_rbrowser_url_is_base_url_when_no_file_sets_it() {
    let layers = Layers {
        base_url: Some("http://localhost:3000".into()),
        environment: Some("local".into()),
        ..Layers::default()
    };
    let resolved = substitute("{{baseUrl}}/health", &layers).expect("resolves");
    assert_eq!(resolved.text, "http://localhost:3000/health");
}

#[test]
fn names_list_secrets_by_name_only() {
    let names = layers().names();
    assert!(names.contains(&("token".to_owned(), true)));
    assert!(names.contains(&("user".to_owned(), false)));
    assert!(names.contains(&("baseUrl".to_owned(), false)));
}

#[test]
fn the_file_takes_strings_numbers_and_booleans_only() {
    let file = EnvFile::parse(r#"{"dev": {"port": 8080, "debug": true}}"#).expect("reads");
    assert_eq!(file.get("dev", "port"), Some("8080"));
    assert_eq!(file.get("dev", "debug"), Some("true"));
    assert_eq!(file.names(), ["dev"]);
    assert!(EnvFile::parse(r#"{"dev": {"a": {"b": 1}}}"#).is_err());
    assert!(EnvFile::parse(r#"{"dev": {"1bad": "x"}}"#).is_err());
    assert!(EnvFile::parse(r#"["dev"]"#).is_err());
    assert!(EnvFile::parse("not json").is_err());
    assert_eq!(
        EnvFile::from_json(&serde_json::Value::Null),
        Ok(EnvFile::default())
    );
}
