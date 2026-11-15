use rb_ipc::{ShellToCore, TrustDecision};
use serde_json::Value;

use crate::trust::tests::{setup, setup_with};
use crate::trust::{ipc, on_start, run_command};

#[tokio::test]
async fn the_plugins_follow_a_decision_and_a_revocation_at_once() {
    let s = setup();
    on_start(&s.core);
    let services = crate::plugins::services::shared();
    services.bind(&s.core);
    assert!(services.local_now().is_empty(), "no own origin untrusted");
    s.decide(TrustDecision::Trust).expect("trusts");
    assert_eq!(
        services.local_now().len(),
        2,
        "startUrl and the environment"
    );
    run_command(&s.core, "trust.revoke", &Value::Null)
        .expect("claimed")
        .expect("revokes");
    assert!(services.local_now().is_empty(), "closed again at once");
}

/// A project with nothing protected: only a start URL on a local port.
fn plain_project(root: &std::path::Path) {
    std::fs::create_dir_all(root.join(".rbrowser")).expect("dir");
    std::fs::write(
        root.join(".rbrowser/config.json"),
        r#"{ "version": 1, "startUrl": "http://127.0.0.1:18765/" }"#,
    )
    .expect("config");
}

#[tokio::test]
async fn a_project_with_nothing_protected_is_trusted_only_on_the_persons_request() {
    let s = setup_with(plain_project);
    on_start(&s.core);
    assert!(
        s.prompts().is_empty(),
        "no banner for a project without protected parts"
    );
    assert!(!s.trusted());

    // The dialog asks, so a person can open the project's own origins to the plugins.
    ipc(&s.core, &ShellToCore::GetTrust)
        .expect("claimed")
        .expect("answers");
    assert_eq!(s.prompts().len(), 1);
    assert_eq!(s.prompts()[0]["blocked"], serde_json::json!([]));
    s.decide(TrustDecision::Trust).expect("trusts");
    assert!(s.trusted());

    run_command(&s.core, "trust.revoke", &Value::Null)
        .expect("claimed")
        .expect("revokes");
    assert!(!s.trusted());
    run_command(&s.core, "trust.grant", &Value::Null)
        .expect("claimed")
        .expect("opens the dialog");
    assert!(!s.trusted(), "the command only asks");
    assert_eq!(
        s.prompts().len(),
        2,
        "asked again under the same id or a new one"
    );
}
