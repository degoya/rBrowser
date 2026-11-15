use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{
    CaptureMode, ChecksRunParams, PointParam, ScreenshotParams, SessionAssignParams,
    SessionClearParams,
};
use crate::backend::McpOptions;
use crate::target::TargetArg;
use crate::testing::{FakeBackend, HOST_TOOL_PREFIX, answer_json, server, view};

#[tokio::test]
async fn screenshot_is_run_by_the_host_with_its_arguments() {
    let fake = Arc::new(FakeBackend::default());
    let params = ScreenshotParams {
        target: Some(TargetArg::all()),
        mode: Some(CaptureMode::FullPage),
        save: Some(false),
        ..ScreenshotParams::default()
    };
    server(&fake)
        .screenshot(Parameters(params))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            format!("{HOST_TOOL_PREFIX}screenshot"),
            json!({ "mode": "fullPage", "save": false, "target": "all" })
        )]
    );
}

/// The element under a point (RB-080-43) reaches the host as `at`, in the page's CSS px.
#[tokio::test]
async fn the_element_under_a_point_reaches_the_host() {
    let fake = Arc::new(FakeBackend::default());
    let params = ScreenshotParams {
        target: Some(TargetArg::Id(1)),
        mode: Some(CaptureMode::Element),
        at: Some(PointParam { x: 12, y: 34 }),
        ..ScreenshotParams::default()
    };
    server(&fake)
        .screenshot(Parameters(params))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            format!("{HOST_TOOL_PREFIX}screenshot"),
            json!({ "mode": "element", "at": { "x": 12, "y": 34 }, "target": 1 })
        )]
    );
}

/// `hideBanners: false` reaches the host as given: the call overrides capture.hideBanners
/// (RB-080-12).
#[tokio::test]
async fn hide_banners_and_freeze_animations_reach_the_host() {
    let fake = Arc::new(FakeBackend::default());
    let params = ScreenshotParams {
        hide_banners: Some(false),
        freeze_animations: Some(true),
        ..ScreenshotParams::default()
    };
    server(&fake)
        .screenshot(Parameters(params))
        .await
        .expect("answer");
    let calls = fake.calls();
    assert_eq!(calls[0].1["hideBanners"], false);
    assert_eq!(calls[0].1["freezeAnimations"], true);
    let unknown = serde_json::from_value::<ScreenshotParams>(json!({ "hideBanner": false }));
    assert!(unknown.is_err(), "deny_unknown_fields");
}

#[tokio::test]
async fn checks_run_runs_checks_run_all_on_every_view_by_default() {
    let fake = Arc::new(FakeBackend::default());
    let params = ChecksRunParams {
        plugins: Some(vec!["seo".to_owned()]),
        ..ChecksRunParams::default()
    };
    server(&fake)
        .checks_run(Parameters(params))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            "checks.run_all".to_owned(),
            json!({ "plugins": ["seo"], "target": "all" })
        )]
    );
}

#[tokio::test]
async fn checks_cancel_runs_the_cancel_command_without_a_target() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake).checks_cancel().await.expect("answer");
    assert_eq!(
        fake.calls(),
        vec![("checks.cancel".to_owned(), serde_json::Value::Null)]
    );
}

#[tokio::test]
async fn the_session_tools_run_the_session_commands() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    let assign = SessionAssignParams {
        target: Some(TargetArg::Name("desktop".to_owned())),
        session: "admin".to_owned(),
    };
    server
        .session_assign(Parameters(assign))
        .await
        .expect("answer");
    let clear = SessionClearParams {
        session: "admin".to_owned(),
    };
    server
        .session_clear(Parameters(clear))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![
            (
                "session.assign".to_owned(),
                json!({ "session": "admin", "target": 2 })
            ),
            ("session.clear".to_owned(), json!({ "session": "admin" })),
        ]
    );
}

fn inline(target: TargetArg) -> ScreenshotParams {
    ScreenshotParams {
        target: Some(target),
        save: Some(false),
        ..ScreenshotParams::default()
    }
}

fn cap(fake: &FakeBackend, max_answer_bytes: usize) {
    fake.set_options(McpOptions {
        max_answer_bytes,
        ..McpOptions::default()
    });
}

/// `save: false` answers the PNGs inline; together, across every call of the fan-out, they are
/// bound to `mcp.maxAnswerBytes` (batch-3 security review, finding 4).
#[tokio::test]
async fn inline_pngs_of_the_whole_fan_out_are_bound_to_max_answer_bytes() {
    let fake = Arc::new(FakeBackend::new(vec![
        view(1, "mobile", "about:blank"),
        view(2, "mobile", "about:blank"),
    ]));
    cap(&fake, 1000);
    let png = "A".repeat(600);
    fake.answer(
        &format!("{HOST_TOOL_PREFIX}screenshot"),
        Ok(json!({ "shots": [{ "view": 1, "pngBase64": png }] })),
    );
    let server = server(&fake);
    let (error, answer) = answer_json(
        &server
            .screenshot(Parameters(inline(TargetArg::Name("mobile".to_owned()))))
            .await
            .expect("answer"),
    );
    assert!(error, "two views of 600 bytes each pass 1000: {answer}");
    assert_eq!(answer["code"], "answer_too_large");
    assert_eq!(answer["settingsKey"], "mcp.maxAnswerBytes");
    let message = answer["error"].as_str().unwrap_or_default();
    assert!(message.contains("save: true"), "{message}");

    // One view fits; the overview counts as well.
    let (error, _) = answer_json(
        &server
            .screenshot(Parameters(inline(TargetArg::Id(1))))
            .await
            .expect("answer"),
    );
    assert!(!error);
    fake.answer(
        &format!("{HOST_TOOL_PREFIX}screenshot"),
        Ok(json!({ "shots": [], "overview": { "pngBase64": "B".repeat(1001) } })),
    );
    let (error, _) = answer_json(
        &server
            .screenshot(Parameters(inline(TargetArg::Id(1))))
            .await
            .expect("answer"),
    );
    assert!(error, "an overview over the limit");
}

#[tokio::test]
async fn saved_screenshots_answer_paths_and_are_not_bound() {
    let fake = Arc::new(FakeBackend::default());
    cap(&fake, 10);
    fake.answer(
        &format!("{HOST_TOOL_PREFIX}screenshot"),
        Ok(json!({ "shots": [{ "view": 1, "path": "/tmp/site/.rbrowser/out/screenshots/a-long-name.png" }] })),
    );
    let (error, _) = answer_json(
        &server(&fake)
            .screenshot(Parameters(ScreenshotParams {
                target: Some(TargetArg::all()),
                ..ScreenshotParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(!error);
}
