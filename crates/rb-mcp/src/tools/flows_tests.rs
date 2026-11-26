use std::sync::Arc;

use rb_config::ViewScope;
use rb_engine::ViewId;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{FlowListParams, FlowRunParams};
use crate::run_target::{RunTarget, TargetItem};
use crate::testing::{FakeBackend, answer_json, server};

fn run(name: &str, target: Option<RunTarget>, wait: Option<bool>) -> Parameters<FlowRunParams> {
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
            Some(RunTarget::Name("desktop".to_owned())),
            Some(false),
        ))
        .await
        .expect("answer");
    server(&fake)
        .flow_run(run("login", Some(RunTarget::Name("all".to_owned())), None))
        .await
        .expect("answer");
    assert_eq!(
        fake.flow_runs(),
        [
            ("login".to_owned(), None, None, true),
            ("login".to_owned(), Some(vec![ViewId(2)]), None, false),
            (
                "login".to_owned(),
                Some(vec![ViewId(1), ViewId(2)]),
                None,
                true
            ),
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
                Some(RunTarget::Name("watch".to_owned())),
                None,
            ))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("view.not_found"));
}

/// RB-099-10: `"visible"` and a list go to the app as the scope it resolves at the start.
#[tokio::test]
async fn flow_run_hands_visible_and_a_list_on_as_the_scope() {
    let fake = Arc::new(FakeBackend::default());
    fake.set_flows(json!({}));
    for target in [
        RunTarget::Name("visible".to_owned()),
        RunTarget::List(vec![
            TargetItem::Id(2),
            TargetItem::Name("tablet".to_owned()),
        ]),
    ] {
        server(&fake)
            .flow_run(run("login", Some(target), None))
            .await
            .expect("answer");
    }
    assert_eq!(
        fake.flow_runs(),
        [
            ("login".to_owned(), None, Some(ViewScope::Visible), true),
            (
                "login".to_owned(),
                None,
                Some(ViewScope::Breakpoints(vec![
                    "desktop".to_owned(),
                    "tablet".to_owned()
                ])),
                true
            ),
        ]
    );
}
