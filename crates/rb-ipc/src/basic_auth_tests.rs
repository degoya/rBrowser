use serde_json::json;

use super::*;
use crate::{CoreToShell, ShellToCore, parse_request};

#[test]
fn a_status_names_the_user_and_never_a_password_field() {
    let stored = EnvironmentAuthStatus::stored("staging", "ada");
    let json = serde_json::to_value(&stored).expect("json");
    assert_eq!(
        json,
        json!({ "environment": "staging", "stored": true, "user": "ada" })
    );
    let none = EnvironmentAuthStatus::none("local");
    assert_eq!(
        serde_json::to_value(&none).expect("json"),
        json!({ "environment": "local", "stored": false })
    );
    let back: EnvironmentAuthStatus = serde_json::from_value(json).expect("reads back");
    assert_eq!(back, stored);
}

#[test]
fn a_password_field_is_refused() {
    let read = serde_json::from_value::<EnvironmentAuthStatus>(json!({
        "environment": "staging", "stored": true, "user": "ada", "password": "x"
    }));
    assert!(read.is_err());
}

#[test]
fn the_query_and_its_answer_have_their_wire_names() {
    assert_eq!(
        parse_request(r#"{"type":"getBasicAuthStatus"}"#).expect("parses"),
        ShellToCore::GetBasicAuthStatus
    );
    let answer = CoreToShell::BasicAuthStatus {
        environments: vec![EnvironmentAuthStatus::stored("staging", "ada")],
        store: CredentialStoreKind::SecretService,
    };
    assert_eq!(
        serde_json::to_value(&answer).expect("json"),
        json!({
            "type": "basicAuthStatus",
            "environments": [{ "environment": "staging", "stored": true, "user": "ada" }],
            "store": "secretService"
        })
    );
}

#[test]
fn each_credential_store_has_its_wire_name() {
    for (kind, name) in [
        (
            CredentialStoreKind::WindowsCredentialManager,
            "windowsCredentialManager",
        ),
        (CredentialStoreKind::MacosKeychain, "macosKeychain"),
        (CredentialStoreKind::SecretService, "secretService"),
        (CredentialStoreKind::None, "none"),
    ] {
        assert_eq!(serde_json::to_value(kind).expect("json"), json!(name));
        let back: CredentialStoreKind = serde_json::from_value(json!(name)).expect("reads back");
        assert_eq!(back, kind);
    }
    assert!(serde_json::from_value::<CredentialStoreKind>(json!("plainFile")).is_err());
}
