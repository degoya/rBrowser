use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{PageZoomLevel, PageZoomParams};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server, view};

fn params(target: Option<TargetArg>, level: Option<PageZoomLevel>) -> PageZoomParams {
    PageZoomParams { target, level }
}

#[tokio::test]
async fn without_a_level_the_page_goes_back_to_100_percent() {
    let fake = Arc::new(FakeBackend::default());
    let (error, _) = answer_json(
        &server(&fake)
            .view_page_zoom(Parameters(PageZoomParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(
        fake.calls(),
        vec![("view.page_zoom.reset".to_owned(), json!({}))],
        "no target: the command's active view"
    );
}

#[tokio::test]
async fn a_factor_and_the_steps_run_their_commands_per_target() {
    let fake = Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "desktop", "https://a.test/"),
    ]));
    server(&fake)
        .view_page_zoom(Parameters(params(
            Some(TargetArg::Id(2)),
            Some(PageZoomLevel::Factor(1.5)),
        )))
        .await
        .expect("answer");
    server(&fake)
        .view_page_zoom(Parameters(params(
            Some(TargetArg::all()),
            Some(PageZoomLevel::Step("in".to_owned())),
        )))
        .await
        .expect("answer");
    server(&fake)
        .view_page_zoom(Parameters(params(
            Some(TargetArg::Name("mobile".to_owned())),
            Some(PageZoomLevel::Step("out".to_owned())),
        )))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            (
                "view.page_zoom.reset".to_owned(),
                json!({ "level": 1.5, "target": 2 })
            ),
            ("view.page_zoom.in".to_owned(), json!({ "target": "all" })),
            ("view.page_zoom.out".to_owned(), json!({ "target": 1 })),
        ]
    );
}

#[tokio::test]
async fn a_level_chromium_does_not_zoom_to_is_refused_before_any_command() {
    let fake = Arc::new(FakeBackend::default());
    for level in [
        PageZoomLevel::Factor(0.1),
        PageZoomLevel::Factor(6.0),
        PageZoomLevel::Factor(f64::NAN),
        PageZoomLevel::Step("fit".to_owned()),
    ] {
        let (error, answer) = answer_json(
            &server(&fake)
                .view_page_zoom(Parameters(params(None, Some(level))))
                .await
                .expect("answer"),
        );
        assert!(error);
        assert_eq!(answer["code"], json!("invalid_arguments"));
    }
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn an_unknown_target_names_the_views_there_are() {
    let fake = Arc::new(FakeBackend::new(vec![view(1, "mobile", "https://a.test/")]));
    let (error, answer) = answer_json(
        &server(&fake)
            .view_page_zoom(Parameters(params(Some(TargetArg::Id(9)), None)))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("view_not_found"));
    assert!(fake.calls().is_empty());
}
