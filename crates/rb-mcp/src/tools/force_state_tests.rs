use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

use super::ForceStateParams;
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server, view};

async fn call(fake: &Arc<FakeBackend>, params: ForceStateParams) -> (bool, Value) {
    answer_json(
        &server(fake)
            .views_force_state(Parameters(params))
            .await
            .expect("answer"),
    )
}

fn two_views() -> Arc<FakeBackend> {
    Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "desktop", "https://a.test/"),
    ]))
}

/// A point in one view, hover in all: one call of the command with `target: "all"`, which
/// finds the same element in the other views.
#[tokio::test]
async fn a_point_forces_the_states_in_the_target_through_one_command() {
    let fake = two_views();
    let (error, _) = call(
        &fake,
        ForceStateParams {
            target: Some(TargetArg::all()),
            x: Some(120.0),
            y: Some(18.5),
            view: Some(1),
            states: Some(vec!["hover".to_owned(), ":focus-within".to_owned()]),
            ..ForceStateParams::default()
        },
    )
    .await;
    assert!(!error);
    assert_eq!(
        fake.calls(),
        vec![(
            "view.force_state.set".to_owned(),
            json!({
                "x": 120.0, "y": 18.5, "view": 1,
                "states": ["hover", ":focus-within"], "target": "all",
            })
        )]
    );
}

#[tokio::test]
async fn a_selector_runs_once_per_resolved_view_and_clear_takes_the_elements_back() {
    let fake = two_views();
    call(
        &fake,
        ForceStateParams {
            target: Some(TargetArg::Name("desktop".to_owned())),
            selector: Some("nav li.menu".to_owned()),
            clear: Some(true),
            ..ForceStateParams::default()
        },
    )
    .await;
    assert_eq!(
        fake.calls(),
        vec![(
            "view.force_state.set".to_owned(),
            json!({ "selector": "nav li.menu", "clear": true, "target": 2 })
        )]
    );
}

/// `clear` without an element resets: every view without a target.
#[tokio::test]
async fn clear_without_an_element_resets_the_target() {
    let fake = two_views();
    call(
        &fake,
        ForceStateParams {
            clear: Some(true),
            ..ForceStateParams::default()
        },
    )
    .await;
    call(
        &fake,
        ForceStateParams {
            target: Some(TargetArg::Id(1)),
            clear: Some(true),
            ..ForceStateParams::default()
        },
    )
    .await;
    assert_eq!(
        fake.calls(),
        vec![
            ("view.force_state.reset".to_owned(), json!({})),
            ("view.force_state.reset".to_owned(), json!({ "target": 1 })),
        ]
    );
}

#[tokio::test]
async fn an_element_is_required_and_the_arguments_must_agree() {
    let fake = two_views();
    for params in [
        ForceStateParams::default(),
        ForceStateParams {
            x: Some(1.0),
            ..ForceStateParams::default()
        },
        ForceStateParams {
            x: Some(1.0),
            y: Some(1.0),
            selector: Some("li".to_owned()),
            ..ForceStateParams::default()
        },
        ForceStateParams {
            selector: Some("li".to_owned()),
            states: Some(Vec::new()),
            clear: Some(true),
            ..ForceStateParams::default()
        },
    ] {
        let (error, answer) = call(&fake, params).await;
        assert!(error);
        assert_eq!(answer["code"], json!("args.invalid"));
    }
    assert!(fake.calls().is_empty(), "nothing ran");
}
