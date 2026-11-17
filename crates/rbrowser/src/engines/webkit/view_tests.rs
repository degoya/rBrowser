use std::sync::Arc;

use rb_config::EngineKind;
use rb_engine::test_support::conformance::sample_spec;
use std::time::Duration;

use rb_engine::external::CellScale;
use rb_engine::{
    Button, CellApplied, Engine, EngineError, HandleEvent, InputEvent, Phase, ViewHandle, ViewId,
};
use rb_sync::{AgentCommand, CommandResult};
use serde_json::json;
use tokio::runtime::Handle;

use crate::engines::webkit::engine::WebkitEngine;
use crate::engines::webkit::fake::FakeSidecar;
use crate::engines::webkit::rpc;

async fn open(fake: &Arc<FakeSidecar>) -> Arc<super::WebkitView> {
    let webkit = WebkitEngine::new(Arc::new(fake.clone()), Handle::current(), "");
    webkit
        .create_view(&sample_spec(EngineKind::Webkit, 5))
        .await
        .expect("opens");
    let view = webkit.view(rb_engine::ViewId(5)).expect("registered");
    // The engine may go; the view keeps working on its own.
    drop(webkit);
    view
}

#[tokio::test]
async fn input_goes_out_in_css_px_with_the_replay_marker_first() {
    let fake = FakeSidecar::new();
    let view = open(&fake).await;
    let click = InputEvent::Mouse {
        phase: Phase::Down,
        x: 12.5,
        y: 40.0,
        button: Button::Left,
        clicks: 1,
        modifiers: 8,
    };
    view.input(
        std::slice::from_ref(&click),
        Some(&AgentCommand::Replay { ms: 300 }),
        1000,
    )
    .await
    .expect("sent");
    assert_eq!(
        fake.calls_of(rpc::INPUT),
        [json!({
            "view": 5,
            "events": [{ "type": "mouse", "phase": "down", "x": 12.5, "y": 40.0, "button": "left", "clicks": 1, "modifiers": 8 }],
            "before": { "cmd": "replay", "ms": 300 },
            "blockSubmitMs": 1000,
        })]
    );
    view.dispatch_input(&InputEvent::Text {
        text: "hi".to_owned(),
    })
    .await
    .expect("sent");
    assert_eq!(
        fake.calls_of(rpc::INPUT)[1],
        json!({ "view": 5, "events": [{ "type": "text", "text": "hi" }] })
    );
}

#[tokio::test]
async fn the_agent_answers_a_resolve() {
    let fake = FakeSidecar::new();
    let view = open(&fake).await;
    fake.answer_agent(json!({ "status": "not_found" }));
    let answer = view.agent(&AgentCommand::Ping).await.expect("answers");
    assert_eq!(answer, CommandResult::NotFound);
    fake.answer_agent(json!({ "nonsense": true }));
    assert!(matches!(
        view.agent(&AgentCommand::Ping).await,
        Err(EngineError::Invalid(_))
    ));
}

#[tokio::test]
async fn emulation_is_applied_and_reported_by_the_webkit_table() {
    let fake = FakeSidecar::new();
    let view = open(&fake).await;
    let mut emulation = sample_spec(EngineKind::Webkit, 5).emulation;
    emulation.platform = Some("iPhone".to_owned());
    let report = view.apply_emulation(&emulation).await.expect("applied");
    assert_eq!(report.unsupported(), ["platform"]);
    assert_eq!(view.emulation().platform.as_deref(), Some("iPhone"));
    assert_eq!(fake.calls_of(rpc::EMULATE)[0]["options"]["isMobile"], true);
}

#[tokio::test]
async fn a_scroll_is_sent_with_the_leaders_range() {
    let fake = FakeSidecar::new();
    let view = open(&fake).await;
    view.scroll(0.0, 640.0, 0.0, 2400.0).await.expect("sent");
    assert_eq!(
        fake.calls_of(rpc::SCROLL),
        [json!({ "view": 5, "x": 0.0, "y": 640.0, "maxX": 0.0, "maxY": 2400.0 })]
    );
}

#[tokio::test]
async fn a_window_closed_meanwhile_is_closed_for_the_view() {
    let fake = FakeSidecar::new();
    let view = open(&fake).await;
    // The sidecar no longer knows the view (its window was closed): the view is closed.
    fake.crash();
    assert!(matches!(
        view.navigate("http://localhost/").await,
        Err(EngineError::Closed)
    ));
    view.close().await.expect("closing a gone window is fine");
}

/// Closing a view ends its events (the external windows' watch task with them) and the engine
/// forgets it (RB-040-04 review, finding 3).
#[tokio::test]
async fn close_ends_the_events_and_the_engine_forgets_the_view() {
    let fake = FakeSidecar::new();
    let webkit = WebkitEngine::new(Arc::new(fake.clone()), Handle::current(), "");
    let handle = webkit
        .create_view(&sample_spec(EngineKind::Webkit, 5))
        .await
        .expect("opens");
    let mut events = handle.events();
    handle.close().await.expect("closes");
    let ended = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
    assert_eq!(
        ended.ok().flatten(),
        Some(HandleEvent::Exited { crashed: false })
    );
    assert!(webkit.view(ViewId(5)).is_none());
    assert_eq!(fake.calls_of(rpc::CLOSE), [json!({ "view": 5 })]);
}

/// In its grid cell (RB-080-40) the context takes the cell's height and factor, the emulation
/// applied again keeps them, a screenshot is taken in CSS px, and a factor WebKit builds a new
/// context for is a new window the app hears of.
#[tokio::test]
async fn a_cell_scale_goes_into_the_context_and_a_rebuilt_context_is_a_replaced_window() {
    let fake = FakeSidecar::new();
    let view = open(&fake).await;
    let mut events = ViewHandle::events(view.as_ref());
    let cell = CellScale {
        factor: 0.82,
        width: 390,
        height: 610,
    };
    let applied = view.set_cell(Some(cell)).await.expect("scaled");
    assert_eq!(applied, CellApplied::default());
    let options = &fake.calls_of(rpc::EMULATE)[0]["options"];
    assert_eq!(options["viewport"], json!({ "width": 390, "height": 610 }));
    assert_eq!(options["deviceScaleFactor"], json!(0.82));
    assert_eq!(view.cell(), Some(cell));
    view.apply_emulation(&view.emulation())
        .await
        .expect("applied");
    assert_eq!(
        fake.calls_of(rpc::EMULATE)[1]["options"]["deviceScaleFactor"],
        json!(0.82),
        "the emulation applied again keeps the cell"
    );
    view.screenshot(rb_engine::ScreenshotRequest { full_page: false })
        .await
        .expect("a screenshot");
    assert_eq!(
        fake.calls_of(rpc::SCREENSHOT)[0],
        json!({ "view": 5, "fullPage": false, "scale": "css" })
    );
    // A new factor WebKit cannot change live: a new context, a new window.
    fake.recreate_next();
    let applied = view
        .set_cell(Some(CellScale {
            factor: 0.5,
            width: 390,
            height: 1000,
        }))
        .await
        .expect("scaled");
    assert!(applied.window_replaced);
    let heard = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
    assert_eq!(heard.ok().flatten(), Some(HandleEvent::WindowReplaced));
    // Out of the cell: the breakpoint's size and ratio, screenshots in device px again.
    view.set_cell(None).await.expect("released");
    let options = &fake.calls_of(rpc::EMULATE)[3]["options"];
    assert_eq!(options["viewport"], json!({ "width": 390, "height": 844 }));
    assert_eq!(options["deviceScaleFactor"], json!(3.0));
    view.screenshot(rb_engine::ScreenshotRequest { full_page: true })
        .await
        .expect("a screenshot");
    assert_eq!(
        fake.calls_of(rpc::SCREENSHOT)[1],
        json!({ "view": 5, "fullPage": true })
    );
}

/// In its cell the page's `devicePixelRatio` is the cell's factor (Playwright's one knob), so
/// the report says the breakpoint's ratio is only approximated (review finding 7).
#[tokio::test]
async fn in_a_cell_the_report_marks_the_pixel_ratio_as_approximated() {
    let fake = FakeSidecar::new();
    let view = open(&fake).await;
    let emulation = view.emulation();
    let report = view.apply_emulation(&emulation).await.expect("applied");
    assert!(!report.unsupported().contains(&"dpr".to_owned()));
    view.set_cell(Some(CellScale {
        factor: 0.82,
        width: 390,
        height: 610,
    }))
    .await
    .expect("scaled");
    let report = view.apply_emulation(&emulation).await.expect("applied");
    assert!(report.unsupported().contains(&"dpr".to_owned()));
    view.set_cell(None).await.expect("released");
    let report = view.apply_emulation(&emulation).await.expect("applied");
    assert!(!report.unsupported().contains(&"dpr".to_owned()));
}
