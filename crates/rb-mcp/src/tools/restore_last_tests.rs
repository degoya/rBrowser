use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::RestoreLastParams;
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server, view};

#[tokio::test]
async fn without_a_target_every_view_takes_its_last_page() {
    let fake = Arc::new(FakeBackend::default());
    let (error, _) = answer_json(
        &server(&fake)
            .views_restore_last(Parameters(RestoreLastParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(
        fake.calls(),
        vec![("views.restore_last".to_owned(), json!({}))],
        "no target: the command's every view"
    );
}

#[tokio::test]
async fn a_view_id_or_a_breakpoint_runs_the_command_per_view() {
    let fake = Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "desktop", "https://a.test/"),
    ]));
    for target in [TargetArg::Id(2), TargetArg::Name("mobile".to_owned())] {
        server(&fake)
            .views_restore_last(Parameters(RestoreLastParams {
                target: Some(target),
            }))
            .await
            .expect("answer");
    }
    assert_eq!(
        fake.calls(),
        vec![
            ("views.restore_last".to_owned(), json!({ "target": 2 })),
            ("views.restore_last".to_owned(), json!({ "target": 1 })),
        ]
    );
}

#[tokio::test]
async fn an_unknown_target_runs_nothing() {
    let fake = Arc::new(FakeBackend::new(vec![view(1, "mobile", "https://a.test/")]));
    let (error, answer) = answer_json(
        &server(&fake)
            .views_restore_last(Parameters(RestoreLastParams {
                target: Some(TargetArg::Id(9)),
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("view.not_found"));
    assert!(fake.calls().is_empty());
}
