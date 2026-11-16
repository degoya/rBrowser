use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{SitemapCaptureParams, SitemapRunParams};
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server};

#[tokio::test]
async fn sitemap_capture_hands_its_arguments_to_the_host_and_answers_the_run() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        &format!("{HOST_TOOL_PREFIX}sitemap_capture"),
        Ok(json!({ "runId": "2026-11-11_10-00-00", "folder": "f", "pending": 76 })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .sitemap_capture(Parameters(SitemapCaptureParams {
                url: Some("https://www.example.test/".to_owned()),
                breakpoints: vec!["mobile".to_owned()],
                env: Some("local".to_owned()),
                resume: None,
                ..SitemapCaptureParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["runId"], "2026-11-11_10-00-00");
    assert_eq!(answer["pending"], 76);
    assert_eq!(
        fake.calls(),
        [(
            format!("{HOST_TOOL_PREFIX}sitemap_capture"),
            json!({ "url": "https://www.example.test/", "breakpoints": ["mobile"], "env": "local" })
        )]
    );
}

#[tokio::test]
async fn status_and_cancel_name_the_run_or_none() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .sitemap_status(Parameters(SitemapRunParams::default()))
        .await
        .expect("answer");
    server
        .sitemap_cancel(Parameters(SitemapRunParams {
            run_id: Some("r1".to_owned()),
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        [
            (format!("{HOST_TOOL_PREFIX}sitemap_status"), json!({})),
            (
                format!("{HOST_TOOL_PREFIX}sitemap_cancel"),
                json!({ "runId": "r1" })
            ),
        ]
    );
}

#[test]
fn unknown_arguments_are_refused() {
    let refused = serde_json::from_value::<SitemapCaptureParams>(json!({ "target": "all" }));
    assert!(refused.is_err());
    let refused = serde_json::from_value::<SitemapRunParams>(json!({ "id": "r1" }));
    assert!(refused.is_err());
}
