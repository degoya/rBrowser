use rb_config::ProjectConfig;
use serde_json::json;

use super::*;
use crate::basic_auth::store::MemoryStore;

fn config(listed: &[&str]) -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "environments": [
            { "id": "staging", "url": "https://staging.example.test/" },
            { "id": "remote", "url": "http://plain.example.test/" }
        ],
        "basicAuth": { "environments": listed }
    }))
    .expect("config")
}

fn staging() -> Challenge<'static> {
    Challenge {
        origin_url: "https://staging.example.test/",
        is_proxy: false,
        host: "staging.example.test",
        port: 443,
        realm: "Staging",
        scheme: "basic",
    }
}

fn service() -> BasicAuth {
    BasicAuth::new(Box::new(MemoryStore::default()))
}

fn credential() -> Credential {
    Credential::new("ada", "service-test-password-1").expect("valid")
}

#[test]
fn a_stored_credential_answers_its_environment_once_per_view() {
    let auth = service();
    let config = config(&["staging"]);
    let origin = auth
        .set("project-a", &config, "staging", &credential())
        .expect("stored");
    assert_eq!(origin.to_string(), "https://staging.example.test:443");
    assert_eq!(
        auth.answer(&staging(), 1, Some("project-a"), &config),
        Ok(credential())
    );
    // The server refused it: the second challenge of the view is the person's.
    assert_eq!(
        auth.answer(&staging(), 1, Some("project-a"), &config),
        Err(Refusal::Refused)
    );
    // Setting it again tries once more.
    auth.set("project-a", &config, "staging", &credential())
        .expect("stored");
    assert!(
        auth.answer(&staging(), 1, Some("project-a"), &config)
            .is_ok()
    );
}

/// RB-094-10: a second realm of the environment's origin is answered once as well.
#[test]
fn a_second_realm_is_answered_once_as_well() {
    let auth = service();
    let config = config(&["staging"]);
    auth.set("project-a", &config, "staging", &credential())
        .expect("stored");
    let admin = Challenge {
        realm: "Admin area",
        ..staging()
    };
    assert!(
        auth.answer(&staging(), 1, Some("project-a"), &config)
            .is_ok()
    );
    assert!(auth.answer(&admin, 1, Some("project-a"), &config).is_ok());
    assert_eq!(
        auth.answer(&admin, 1, Some("project-a"), &config),
        Err(Refusal::Refused)
    );
}

#[test]
fn nothing_is_answered_without_the_flag_the_credential_or_the_project() {
    let auth = service();
    auth.set("project-a", &config(&[]), "staging", &credential())
        .expect("a credential may be set before the flag");
    assert_eq!(
        auth.answer(&staging(), 1, Some("project-a"), &config(&[])),
        Err(Refusal::NotAnEnvironment)
    );
    assert_eq!(
        auth.answer(&staging(), 1, Some("project-b"), &config(&["staging"])),
        Err(Refusal::NoCredential),
        "another project's credential is not this one's"
    );
    assert_eq!(
        auth.answer(&staging(), 1, None, &config(&["staging"])),
        Err(Refusal::NoProject)
    );
}

#[test]
fn a_credential_is_set_only_for_a_known_environment_that_may_carry_one() {
    let auth = service();
    assert_eq!(
        auth.set("p", &config(&["staging"]), "nope", &credential()),
        Err(AuthError::UnknownEnvironment("nope".to_owned()))
    );
    assert_eq!(
        auth.set("p", &config(&["remote"]), "remote", &credential()),
        Err(AuthError::Origin(OriginError::PlainHttp))
    );
}

#[test]
fn a_removed_credential_answers_nothing_more() {
    let auth = service();
    let config = config(&["staging"]);
    auth.set("p", &config, "staging", &credential())
        .expect("stored");
    assert_eq!(auth.remove("p", &config, "staging"), Ok(true));
    assert_eq!(auth.remove("p", &config, "staging"), Ok(false));
    assert_eq!(
        auth.answer(&staging(), 1, Some("p"), &config),
        Err(Refusal::NoCredential)
    );
}

#[test]
fn a_moved_environment_does_not_take_its_credential_along() {
    // MCP may edit `environments` (not `basicAuth`): pointing staging at another host must not
    // send staging's credential there.
    let auth = service();
    auth.set("p", &config(&["staging"]), "staging", &credential())
        .expect("stored");
    let moved: ProjectConfig = serde_json::from_value(json!({
        "version": 1,
        "environments": [{ "id": "staging", "url": "https://evil.example.test/" }],
        "basicAuth": { "environments": ["staging"] }
    }))
    .expect("config");
    let evil = Challenge {
        origin_url: "https://evil.example.test/",
        host: "evil.example.test",
        ..staging()
    };
    assert_eq!(
        auth.answer(&evil, 1, Some("p"), &moved),
        Err(Refusal::NoCredential)
    );
}

#[test]
fn the_password_is_masked_in_the_log_once_known() {
    let auth = service();
    let secret = "masked-after-set-0815";
    auth.set(
        "p",
        &config(&["staging"]),
        "staging",
        &Credential::new("ada", secret).expect("valid"),
    )
    .expect("stored");
    let line = rb_diag::redact(&format!("a line naming {secret}"));
    assert!(!line.contains(secret), "{line}");
}
