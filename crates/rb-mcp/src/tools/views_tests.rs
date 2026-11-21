use std::sync::Arc;

use rb_commands::CommandError;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{
    EngineArg, NavigateParams, PopOutParams, ReloadMode, ReloadParams, SetEngineParams,
    TargetParams,
};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server};

#[tokio::test]
async fn views_list_answers_every_view() {
    let fake = Arc::new(FakeBackend::default());
    let (error, answer) = answer_json(&server(&fake).views_list().await.expect("answer"));
    assert!(!error);
    assert_eq!(answer[0]["id"], 1);
    assert_eq!(answer[1]["breakpoint"], "desktop");
}

#[tokio::test]
async fn navigate_all_runs_the_command_once_and_answers_each_final_url() {
    let fake = Arc::new(FakeBackend::default());
    let params = NavigateParams {
        url: "https://b.test/".to_owned(),
        target: Some(TargetArg::all()),
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .navigate(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["settled"], true);
    assert_eq!(answer["views"][0]["url"], "https://b.test/");
    assert_eq!(answer["views"][1]["url"], "https://b.test/");
    assert_eq!(
        fake.calls(),
        vec![(
            "view.navigate".to_owned(),
            json!({ "url": "https://b.test/", "target": "all" })
        )]
    );
}

#[tokio::test]
async fn navigate_one_breakpoint_reaches_only_its_view() {
    let fake = Arc::new(FakeBackend::default());
    let params = NavigateParams {
        url: "https://c.test/".to_owned(),
        target: Some(TargetArg::Name("mobile".to_owned())),
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .navigate(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(1));
    assert_eq!(fake.url_of(1).as_deref(), Some("https://c.test/"));
    assert_eq!(fake.url_of(2).as_deref(), Some("about:blank"));
    assert_eq!(fake.calls()[0].1["target"], 1);
}

#[tokio::test]
async fn an_unknown_target_runs_nothing_and_names_the_views() {
    let fake = Arc::new(FakeBackend::default());
    let params = NavigateParams {
        url: "https://c.test/".to_owned(),
        target: Some(TargetArg::Name("foo".to_owned())),
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .navigate(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], "view_not_found");
    assert_eq!(
        answer["error"],
        "view `foo` not found; known views: 1 (mobile), 2 (desktop)"
    );
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn reload_back_and_forward_run_their_commands() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    let reload = ReloadParams {
        target: Some(TargetArg::Id(2)),
        ignore_cache: true,
        mode: None,
    };
    server.reload(Parameters(reload)).await.expect("answer");
    server
        .back(Parameters(TargetParams::default()))
        .await
        .expect("answer");
    server
        .forward(Parameters(TargetParams::default()))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            (
                "reload.all".to_owned(),
                json!({ "ignoreCache": true, "target": 2 })
            ),
            ("view.back".to_owned(), json!({ "target": "all" })),
            ("view.forward".to_owned(), json!({ "target": "all" })),
        ]
    );
}

#[tokio::test]
async fn reload_css_swaps_the_stylesheets_with_its_own_command() {
    // RB-030-05: `mode: "css"` is hot reload's `reload.css_only`; the pages stay.
    let fake = Arc::new(FakeBackend::default());
    fake.answer("reload.css_only", Ok(json!({ "mode": "css", "views": [] })));
    let reload = ReloadParams {
        target: Some(TargetArg::Name("mobile".to_owned())),
        ignore_cache: false,
        mode: Some(ReloadMode::Css),
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .reload(Parameters(reload))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["mode"], "css");
    assert_eq!(
        fake.calls(),
        vec![("reload.css_only".to_owned(), json!({ "target": 1 }))]
    );
}

#[tokio::test]
async fn a_failed_command_is_an_error_answer() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        "view.back",
        Err(CommandError::Failed(
            "the browser is not available".to_owned(),
        )),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .back(Parameters(TargetParams::default()))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], "command_failed");
}

/// Review batch 3, finding 2: `navigate` with `"all"` names the views it left alone and waits
/// only on the ones it reached - a skipped view keeps its URL and does not count as loaded.
#[tokio::test]
async fn navigate_all_names_the_skipped_views_and_waits_on_the_reached_ones() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        "view.navigate",
        Ok(json!({ "views": [1], "skipped": [2], "failed": [] })),
    );
    // View 1 is still loading the new page: not settled within the timeout.
    fake.set_loading(1, true);
    let params = NavigateParams {
        url: "https://b.test/".to_owned(),
        target: Some(TargetArg::all()),
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .navigate(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["skipped"], json!([2]));
    assert_eq!(answer["views"].as_array().map(Vec::len), Some(1));
    assert_eq!(answer["views"][0]["view"], 1);
    assert_eq!(answer["settled"], false);
    assert!(answer.get("failed").is_none(), "only when one failed");
}

#[tokio::test]
async fn a_view_the_navigation_failed_in_is_named() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        "view.back",
        Ok(json!({ "views": [2], "skipped": [], "failed": [{ "view": 1, "error": "gone" }] })),
    );
    let (error, answer) = answer_json(
        &server(&fake)
            .back(Parameters(TargetParams::default()))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["failed"], json!([{ "view": 1, "error": "gone" }]));
    assert_eq!(answer["skipped"], json!([]));
    assert_eq!(answer["views"][0]["view"], 2);
}

#[tokio::test]
async fn view_pop_out_and_in_pass_the_one_view() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .view_pop_out(Parameters(PopOutParams {
            target: Some(TargetArg::Name("desktop".to_owned())),
        }))
        .await
        .expect("answer");
    server
        .view_pop_in(Parameters(PopOutParams::default()))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            ("view.pop_out".to_owned(), json!({ "view": 2 })),
            ("view.pop_in".to_owned(), json!({})),
        ]
    );
}

#[tokio::test]
async fn view_pop_out_refuses_all() {
    let fake = Arc::new(FakeBackend::default());
    let (error, _) = answer_json(
        &server(&fake)
            .view_pop_out(Parameters(PopOutParams {
                target: Some(TargetArg::all()),
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn views_set_engine_runs_the_command_for_one_view_or_every_target() {
    let fake = Arc::new(FakeBackend::default());
    let params = SetEngineParams {
        target: Some(TargetArg::Name("mobile".to_owned())),
        engine: EngineArg::Firefox,
        file: None,
        ios_device: None,
    };
    let (error, _) = answer_json(
        &server(&fake)
            .views_set_engine(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(
        fake.calls(),
        vec![(
            "view.set_engine".to_owned(),
            json!({ "view": 1, "engine": "firefox" })
        )]
    );
    let all = SetEngineParams {
        target: Some(TargetArg::all()),
        engine: EngineArg::Webkit,
        file: Some("project".to_owned()),
        ios_device: None,
    };
    let (error, answer) = answer_json(
        &server(&fake)
            .views_set_engine(Parameters(all))
            .await
            .expect("answer"),
    );
    assert!(!error, "{answer}");
    let calls = fake.calls();
    assert_eq!(calls.len(), 2, "every view in one command: {calls:?}");
    assert_eq!(
        calls[1],
        (
            "view.set_engine".to_owned(),
            json!({ "views": [1, 2], "engine": "webkit", "file": "project" })
        )
    );
    // Without a target the active view is the command's business.
    let active = SetEngineParams {
        target: None,
        engine: EngineArg::Chromium,
        file: None,
        ios_device: None,
    };
    let (error, _) = answer_json(
        &server(&fake)
            .views_set_engine(Parameters(active))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(
        fake.calls()[2].1,
        json!({ "engine": "chromium" }),
        "no view in the arguments"
    );
    let refused: Result<SetEngineParams, _> = serde_json::from_value(json!({ "engine": "opera" }));
    assert!(refused.is_err());
}

#[tokio::test]
async fn views_set_engine_hands_the_ios_device_to_the_command() {
    let fake = Arc::new(FakeBackend::default());
    let params: SetEngineParams =
        serde_json::from_value(json!({ "engine": "ios", "iosDevice": "iPhone 16" }))
            .expect("parses");
    let (error, _) = answer_json(
        &server(&fake)
            .views_set_engine(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(
        fake.calls()[0].1,
        json!({ "engine": "ios", "iosDevice": "iPhone 16" })
    );
}
