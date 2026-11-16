use std::sync::Arc;

use rb_engine::ViewId;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{FlowListParams, FlowRunParams};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server};

fn run(name: &str, target: Option<TargetArg>, wait: Option<bool>) -> Parameters<FlowRunParams> {
    Parameters(FlowRunParams {
        name: name.to_owned(),
        target,
        wait,
    })
}

#[tokio::test]
async fn flow_list_answers_the_backends_list() {
    let fake = Arc::new(FakeBackend::default());
    fake.set_flows(json!({ "trusted": false, "flows": [{ "name": "login" }] }));
    let (error, answer) = answer_json(
        &server(&fake)
            .flow_list(Parameters(FlowListParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(answer["flows"][0]["name"], json!("login"));
}

#[tokio::test]
async fn flow_run_waits_by_default_and_resolves_its_target() {
    let fake = Arc::new(FakeBackend::default());
    fake.set_flows(json!({}));
    server(&fake)
        .flow_run(run("login", None, None))
        .await
        .expect("answer");
    server(&fake)
        .flow_run(run(
            "login",
            Some(TargetArg::Name("desktop".to_owned())),
            Some(false),
        ))
        .await
        .expect("answer");
    server(&fake)
        .flow_run(run("login", Some(TargetArg::all()), None))
        .await
        .expect("answer");
    assert_eq!(
        fake.flow_runs(),
        [
            ("login".to_owned(), None, true),
            ("login".to_owned(), Some(vec![ViewId(2)]), false),
            ("login".to_owned(), Some(vec![ViewId(1), ViewId(2)]), true),
        ]
    );
}

#[tokio::test]
async fn an_untrusted_workspace_is_refused_with_its_code() {
    let fake = Arc::new(FakeBackend::default());
    let (error, answer) = answer_json(
        &server(&fake)
            .flow_run(run("login", None, None))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("flows.untrusted"));
    let (error, answer) = answer_json(
        &server(&fake)
            .flow_run(run(
                "login",
                Some(TargetArg::Name("watch".to_owned())),
                None,
            ))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("view_not_found"));
}
