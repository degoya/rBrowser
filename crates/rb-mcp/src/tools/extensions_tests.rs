use std::sync::Arc;

use serde_json::json;

use super::{ExtensionInstallParams, ExtensionRemoveParams};
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server, view};

fn host(name: &str) -> String {
    format!("{HOST_TOOL_PREFIX}{name}")
}

/// The list goes to the host as it is, without arguments; it changes nothing.
#[tokio::test]
async fn the_list_is_the_hosts_answer() {
    let fake = Arc::new(FakeBackend::new(vec![view(1, "mobile", "https://a.test/")]));
    fake.answer(
        &host("extension_list"),
        Ok(json!({ "enabled": true, "restartNeeded": false, "extensions": [] })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .extension_list(crate::tools::no_args())
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["enabled"], true);
    assert_eq!(fake.calls(), [(host("extension_list"), json!({}))]);
}

/// The install goes to the host with the reference as written; the host asks the person
/// (RB-080-23). No tool loads an unpacked folder or removes an extension.
#[tokio::test]
async fn the_install_is_the_hosts_after_the_persons_word() {
    let fake = Arc::new(FakeBackend::new(vec![view(1, "mobile", "https://a.test/")]));
    fake.answer(
        &host("extension_install"),
        Ok(json!({ "id": "nhdogjmejiglipccpnnnanhbledajbpd", "version": "7.6.4", "restartNeeded": true })),
    );
    let params = ExtensionInstallParams {
        id: "nhdogjmejiglipccpnnnanhbledajbpd".to_owned(),
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .extension_install(rmcp::handler::server::wrapper::Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["restartNeeded"], true);
    assert_eq!(
        fake.calls(),
        [(
            host("extension_install"),
            json!({ "id": "nhdogjmejiglipccpnnnanhbledajbpd" })
        )]
    );
    let names = crate::RbMcpServer::tool_names();
    let ours: Vec<&String> = names
        .iter()
        .filter(|name| name.starts_with("extension"))
        .collect();
    assert_eq!(
        ours,
        ["extension_install", "extension_list", "extension_remove"]
    );
}

/// The removal goes to the host with the reference as written; the host names the extension
/// in the window and deletes it only on the person's word (RB-080-30).
#[tokio::test]
async fn the_removal_is_the_hosts_after_the_persons_word() {
    let fake = Arc::new(FakeBackend::new(vec![view(1, "mobile", "https://a.test/")]));
    fake.answer(
        &host("extension_remove"),
        Ok(json!({ "id": "nhdogjmejiglipccpnnnanhbledajbpd", "name": "Vue.js devtools", "version": "7.6.4", "restartNeeded": true })),
    );
    let params = ExtensionRemoveParams {
        id: "https://chromewebstore.google.com/detail/vue/nhdogjmejiglipccpnnnanhbledajbpd"
            .to_owned(),
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .extension_remove(rmcp::handler::server::wrapper::Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["restartNeeded"], true);
    assert_eq!(
        fake.calls(),
        [(
            host("extension_remove"),
            json!({ "id": "https://chromewebstore.google.com/detail/vue/nhdogjmejiglipccpnnnanhbledajbpd" })
        )]
    );
}
