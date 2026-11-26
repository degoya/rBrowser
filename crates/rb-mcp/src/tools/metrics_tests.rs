use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{VIEW_METRICS_TOOL, ViewMetricsParams};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server, view};

fn fake() -> Arc<FakeBackend> {
    Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "http://a.test/"),
        view(2, "desktop", "http://a.test/"),
        view(3, "mobile", "http://a.test/"),
    ]))
}

fn host_id() -> String {
    format!("{HOST_TOOL_PREFIX}{VIEW_METRICS_TOOL}")
}

#[tokio::test]
async fn all_views_by_default_and_the_host_answers() {
    let fake = fake();
    fake.answer(&host_id(), Ok(json!({ "views": [{ "view": 1 }] })));
    let (error, answer) = answer_json(
        &server(&fake)
            .view_metrics(Parameters(ViewMetricsParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(answer, json!({ "views": [{ "view": 1 }] }));
    assert_eq!(
        fake.calls(),
        vec![(host_id(), json!({ "views": [1, 2, 3] }))]
    );
}

#[tokio::test]
async fn a_breakpoint_names_its_views() {
    let fake = fake();
    fake.answer(&host_id(), Ok(json!({ "views": [] })));
    server(&fake)
        .view_metrics(Parameters(ViewMetricsParams {
            target: Some(TargetArg::Name("mobile".to_owned())),
        }))
        .await
        .expect("answer");
    assert_eq!(fake.calls(), vec![(host_id(), json!({ "views": [1, 3] }))]);
}

#[tokio::test]
async fn an_unknown_target_is_refused_before_the_host() {
    let fake = fake();
    let (error, answer) = answer_json(
        &server(&fake)
            .view_metrics(Parameters(ViewMetricsParams {
                target: Some(TargetArg::Id(9)),
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("view.not_found"));
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn the_answer_passes_the_mask() {
    let fake = fake();
    fake.answer(
        &host_id(),
        Ok(json!({ "views": [{ "url": "http://a.test/?token=abc" }] })),
    );
    let (_, answer) = answer_json(
        &server(&fake)
            .view_metrics(Parameters(ViewMetricsParams::default()))
            .await
            .expect("answer"),
    );
    assert!(answer["views"][0]["url"].is_string());
}
