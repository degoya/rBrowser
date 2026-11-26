use serde_json::json;

use super::*;
use crate::{CoreToShell, ShellToCore, ViewId, parse_request};

fn prompt() -> AuthPrompt {
    AuthPrompt {
        id: 3,
        origin: "https://dev.example.test:443".to_owned(),
        realm: "Staging".to_owned(),
        proxy: false,
        views: vec![ViewId(1), ViewId(2)],
        environment: Some("development".to_owned()),
        refused: false,
        insecure: false,
    }
}

#[test]
fn the_question_reaches_the_shell_beside_its_type() {
    let json = serde_json::to_value(CoreToShell::AuthPrompt(prompt())).expect("json");
    assert_eq!(
        json,
        json!({
            "type": "authPrompt", "id": 3, "origin": "https://dev.example.test:443",
            "realm": "Staging", "proxy": false, "views": [1, 2],
            "environment": "development", "refused": false, "insecure": false
        })
    );
    let back: CoreToShell = serde_json::from_value(json).expect("reads back");
    assert_eq!(back, CoreToShell::AuthPrompt(prompt()));
    let closed = serde_json::to_value(CoreToShell::AuthPromptClosed { id: 3 }).expect("json");
    assert_eq!(closed, json!({ "type": "authPromptClosed", "id": 3 }));
}

#[test]
fn a_question_without_an_environment_or_realm_leaves_them_out() {
    let proxy = AuthPrompt {
        realm: String::new(),
        proxy: true,
        environment: None,
        ..prompt()
    };
    let json = serde_json::to_value(&proxy).expect("json");
    assert_eq!(json.get("environment"), None);
    assert_eq!(json.get("realm"), None);
    let back: AuthPrompt = serde_json::from_value(json).expect("reads back");
    assert_eq!(back, proxy);
}

#[test]
fn the_answer_parses_and_a_cancel_carries_no_credential() {
    let signed = parse_request(
        &json!({
            "type": "authPromptReply", "id": 3,
            "signIn": { "user": "ada", "password": "reply-test-password", "save": true }
        })
        .to_string(),
    )
    .expect("parses");
    let ShellToCore::AuthPromptReply(reply) = &signed else {
        panic!("a reply: {signed:?}");
    };
    assert_eq!(reply.id, 3);
    let sign_in = reply.sign_in.as_ref().expect("signs in");
    assert_eq!(
        (
            sign_in.user.as_str(),
            sign_in.password.as_str(),
            sign_in.save
        ),
        ("ada", "reply-test-password", true)
    );
    let cancel =
        parse_request(&json!({ "type": "authPromptReply", "id": 4 }).to_string()).expect("parses");
    assert_eq!(
        serde_json::to_value(&cancel).expect("json"),
        json!({ "type": "authPromptReply", "id": 4 })
    );
}

/// The IPC layer writes a message it cannot handle with `{:?}` into its failure: neither the user
/// nor the password may show there.
#[test]
fn the_debug_output_never_shows_the_user_or_the_password() {
    let reply = ShellToCore::AuthPromptReply(AuthPromptReply {
        id: 5,
        sign_in: Some(AuthPromptSignIn {
            user: "debug-test-user".to_owned(),
            password: "debug-test-password".to_owned(),
            save: false,
        }),
    });
    let text = format!("{reply:?} / {reply:#?}");
    assert!(!text.contains("debug-test-user"), "{text}");
    assert!(!text.contains("debug-test-password"), "{text}");
    assert!(text.contains('5'), "the id stays: {text}");
}

#[test]
fn unknown_fields_are_refused() {
    let read = serde_json::from_value::<AuthPromptReply>(json!({
        "id": 1, "signIn": { "user": "a", "password": "b-1234", "environment": "x" }
    }));
    assert!(
        read.is_err(),
        "the shell never names the environment to save for"
    );
}
