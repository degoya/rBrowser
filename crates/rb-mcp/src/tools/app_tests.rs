use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{DevtoolsParams, DiagnosticsParams, WslParams};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server};

#[tokio::test]
async fn app_info_answers_the_facts() {
    let fake = Arc::new(FakeBackend::default());
    let (error, answer) = answer_json(&server(&fake).app_info().await.expect("answer"));
    assert!(!error);
    assert_eq!(answer["name"], "rBrowser");
}

#[tokio::test]
async fn diagnostics_never_opens_explorer() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .diagnostics(Parameters(DiagnosticsParams::default()))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![("diagnostics.create".to_owned(), json!({}))]
    );
}

#[tokio::test]
async fn devtools_open_targets_one_view() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .devtools_open(Parameters(DevtoolsParams {
            target: Some(TargetArg::Name("mobile".to_owned())),
            mode: Some("window".to_owned()),
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            "devtools.open".to_owned(),
            json!({ "target": 1, "mode": "window" })
        )]
    );
}

#[tokio::test]
async fn wsl_info_restarts_only_when_asked() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    let (_, info) = answer_json(
        &server
            .wsl_info(Parameters(WslParams::default()))
            .await
            .expect("answer"),
    );
    assert_eq!(info["helper"], "stopped");
    assert!(fake.calls().is_empty());
    server
        .wsl_info(Parameters(WslParams {
            action: Some("restart".to_owned()),
        }))
        .await
        .expect("answer");
    assert_eq!(fake.calls()[0].0, "wsl.restart_helper");
    let (error, _) = answer_json(
        &server
            .wsl_info(Parameters(WslParams {
                action: Some("format".to_owned()),
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
}

#[tokio::test]
async fn plugins_list_runs_its_command() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake).plugins_list().await.expect("answer");
    assert_eq!(fake.calls()[0].0, "plugins.list");
}
