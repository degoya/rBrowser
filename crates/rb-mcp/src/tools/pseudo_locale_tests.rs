use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::PseudoLocaleParams;
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server, view};

#[tokio::test]
async fn without_arguments_the_active_view_is_pseudo_localised() {
    let fake = Arc::new(FakeBackend::default());
    let (error, _) = answer_json(
        &server(&fake)
            .pseudo_locale(Parameters(PseudoLocaleParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(
        fake.calls(),
        vec![("overlays.pseudo_locale".to_owned(), json!({ "on": true }))]
    );
}

#[tokio::test]
async fn the_values_and_the_target_reach_the_command() {
    let fake = Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "desktop", "https://a.test/"),
    ]));
    server(&fake)
        .pseudo_locale(Parameters(PseudoLocaleParams {
            target: Some(TargetArg::all()),
            on: None,
            factor: Some(2.0),
            accents: Some(false),
            rtl: Some(true),
        }))
        .await
        .expect("answer");
    server(&fake)
        .pseudo_locale(Parameters(PseudoLocaleParams {
            target: Some(TargetArg::Id(2)),
            on: Some(false),
            ..PseudoLocaleParams::default()
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            (
                "overlays.pseudo_locale".to_owned(),
                json!({ "on": true, "factor": 2.0, "accents": false, "rtl": true, "target": "all" })
            ),
            (
                "overlays.pseudo_locale".to_owned(),
                json!({ "on": false, "target": 2 })
            ),
        ]
    );
}

#[tokio::test]
async fn a_factor_outside_one_to_three_is_refused_before_any_command() {
    let fake = Arc::new(FakeBackend::default());
    for factor in [0.5, 3.5, f64::NAN] {
        let (error, answer) = answer_json(
            &server(&fake)
                .pseudo_locale(Parameters(PseudoLocaleParams {
                    factor: Some(factor),
                    ..PseudoLocaleParams::default()
                }))
                .await
                .expect("answer"),
        );
        assert!(error);
        assert_eq!(answer["code"], json!("args.invalid"));
    }
    assert!(fake.calls().is_empty());
}
