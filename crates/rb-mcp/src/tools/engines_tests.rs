use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{ENGINES_LIST_TOOL, EngineRestartParams, ExternalEngine};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server, view};

fn params(engine: ExternalEngine, target: Option<TargetArg>) -> Parameters<EngineRestartParams> {
    Parameters(EngineRestartParams { engine, target })
}

#[tokio::test]
async fn engine_restart_runs_engine_restart_for_the_engine_or_each_view() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    for call in [
        params(ExternalEngine::Firefox, None),
        params(ExternalEngine::Webkit, Some(TargetArg::all())),
        params(ExternalEngine::Webkit, Some(TargetArg::Id(2))),
    ] {
        let (error, _) = answer_json(&server.engine_restart(call).await.expect("answer"));
        assert!(!error);
    }
    assert_eq!(
        fake.calls(),
        vec![
            ("engine.restart".to_owned(), json!({ "engine": "firefox" })),
            ("engine.restart".to_owned(), json!({ "engine": "webkit" })),
            (
                "engine.restart".to_owned(),
                json!({ "engine": "webkit", "view": 2 })
            ),
        ]
    );
}

#[tokio::test]
async fn a_breakpoint_shown_twice_restarts_both_views_and_joins_the_answer() {
    let fake = Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "about:blank"),
        view(3, "mobile", "about:blank"),
    ]));
    fake.answer("engine.restart", Ok(json!({ "restarted": [7] })));
    let (error, answer) = answer_json(
        &server(&fake)
            .engine_restart(params(
                ExternalEngine::Firefox,
                Some(TargetArg::Name("mobile".to_owned())),
            ))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(answer, json!({ "restarted": [7, 7] }));
    let views: Vec<_> = fake
        .calls()
        .into_iter()
        .map(|(_, args)| args["view"].clone())
        .collect();
    assert_eq!(views, vec![json!(1), json!(3)]);
}

#[tokio::test]
async fn an_unknown_view_is_refused_before_anything_restarts() {
    let fake = Arc::new(FakeBackend::default());
    let (error, answer) = answer_json(
        &server(&fake)
            .engine_restart(params(ExternalEngine::Firefox, Some(TargetArg::Id(9))))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], json!("view_not_found"));
    assert!(fake.calls().is_empty());
}

#[test]
fn only_firefox_and_webkit_restart() {
    let parsed: EngineRestartParams =
        serde_json::from_value(json!({ "engine": "webkit", "target": "all" })).expect("webkit");
    assert_eq!(parsed.engine, ExternalEngine::Webkit);
    for refused in [
        json!({ "engine": "chromium" }),
        json!({}),
        json!({ "engine": "firefox", "viewId": 2 }),
    ] {
        assert!(
            serde_json::from_value::<EngineRestartParams>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[tokio::test]
async fn engines_list_is_the_hosts_read_of_the_installed_firefox_editions() {
    let fake = Arc::new(FakeBackend::default());
    let listed = json!({
        "firefox": {
            "configured": null,
            "installed": [
                { "path": "C:\\Program Files\\Mozilla Firefox\\firefox.exe", "kind": "release", "version": "134.0" }
            ]
        }
    });
    fake.answer(
        &format!("{HOST_TOOL_PREFIX}{ENGINES_LIST_TOOL}"),
        Ok(listed.clone()),
    );
    let (error, answer) = answer_json(&server(&fake).engines_list().await.expect("answer"));
    assert!(!error, "{answer}");
    assert_eq!(answer, listed);
    assert_eq!(
        fake.calls(),
        vec![(format!("{HOST_TOOL_PREFIX}{ENGINES_LIST_TOOL}"), json!({}))]
    );
}
