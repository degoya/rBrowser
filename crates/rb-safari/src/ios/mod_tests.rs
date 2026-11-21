//! The iOS engine over the fake `safaridriver` and the fake Mac tools.

use std::sync::Arc;

use rb_engine::test_support::conformance::{check_engine, sample_spec};
use rb_engine::{
    Button, Emulation, Engine, EngineError, EngineKind, FieldSupport, InputEvent, Phase,
};

use super::fake::{DEVICE, FakeIosTools, SIMULATOR};
use super::*;
use crate::SafariEngine;
use crate::driver::Attached;
use crate::testing::FakeSafari;

const BUNDLE: &str = "globalThis.__rbAgent = { command: () => ({ status: 'done' }) };";

fn engine(fake: &FakeSafari, tools: &Arc<FakeIosTools>) -> Arc<SafariEngine> {
    SafariEngine::ios(
        Arc::new(Attached(fake.port())),
        tools.clone() as Arc<dyn IosTools>,
        BUNDLE,
    )
}

/// A fake whose windows show an iPhone's 390x844 viewport, as the sample spec asks.
async fn phone_fake() -> FakeSafari {
    let fake = FakeSafari::start().await;
    fake.screen(390, 844);
    fake
}

#[tokio::test]
async fn ios_passes_the_engine_contract_on_the_simulator() {
    let fake = phone_fake().await;
    let tools = Arc::new(FakeIosTools::with_xcode());
    let engine = engine(&fake, &tools);
    assert_eq!(engine.kind(), EngineKind::Ios);
    check_engine(engine.as_ref(), &sample_spec(EngineKind::Ios, 7)).await;
    assert_eq!(
        tools.booted(),
        [SIMULATOR],
        "the simulator was booted first"
    );
}

#[tokio::test]
async fn the_session_asks_for_the_simulator_and_sizes_no_window() {
    let fake = phone_fake().await;
    let tools = Arc::new(FakeIosTools::with_xcode());
    let engine = engine(&fake, &tools);
    let mut spec = sample_spec(EngineKind::Ios, 7);
    spec.breakpoint.ios_device = Some("iPhone 16".to_owned());
    let view = engine.create_view(&spec).await.expect("a view");
    assert_eq!(view.engine(), EngineKind::Ios);
    let asked = &fake.requests_to("/session")[0].body["capabilities"]["alwaysMatch"];
    assert_eq!(asked["platformName"], "iOS");
    assert_eq!(asked["safari:useSimulator"], true);
    assert_eq!(asked["safari:deviceUDID"], SIMULATOR);
    assert!(
        fake.requests_to("/window/rect").is_empty(),
        "an iPhone has no window to size or place"
    );
    let id = fake.session_ids().pop().expect("a session");
    assert_eq!(fake.url_of(&id).as_deref(), Some("http://localhost/start"));
}

#[tokio::test]
async fn without_xcode_the_view_runs_on_the_connected_device() {
    let fake = phone_fake().await;
    let tools = Arc::new(FakeIosTools::with_device());
    let engine = engine(&fake, &tools);
    engine
        .create_view(&sample_spec(EngineKind::Ios, 7))
        .await
        .expect("a view");
    let asked = &fake.requests_to("/session")[0].body["capabilities"]["alwaysMatch"];
    assert_eq!(asked["safari:useSimulator"], false);
    assert_eq!(asked["safari:deviceUDID"], DEVICE);
    assert!(tools.booted().is_empty(), "a device is not booted");
}

#[tokio::test]
async fn without_xcode_and_without_a_device_the_view_says_both_ways() {
    let fake = phone_fake().await;
    let tools = Arc::new(FakeIosTools::default());
    let engine = engine(&fake, &tools);
    let Err(EngineError::Unavailable {
        engine: kind,
        reason,
    }) = engine.create_view(&sample_spec(EngineKind::Ios, 7)).await
    else {
        panic!("unavailable");
    };
    assert_eq!(kind, EngineKind::Ios);
    assert!(
        reason.contains("Xcode") || reason == NoTarget::NoWay.key(),
        "{reason}"
    );
    assert!(fake.requests().is_empty(), "no session was asked for");
    assert_eq!(
        last_inventory().map(|found| found.xcode),
        Some(false),
        "the inventory is kept for the badge menu"
    );
}

#[tokio::test]
async fn a_simulator_that_does_not_boot_is_the_view_s_reason() {
    let fake = phone_fake().await;
    let tools = Arc::new(FakeIosTools::with_xcode());
    tools.fail_boot("the simulator did not boot");
    let engine = engine(&fake, &tools);
    let result = engine.create_view(&sample_spec(EngineKind::Ios, 7)).await;
    assert!(
        matches!(&result, Err(EngineError::Unavailable { engine: EngineKind::Ios, reason }) if reason.contains("did not boot")),
        "{:?}",
        result.err()
    );
}

#[tokio::test]
async fn the_report_names_the_device_and_the_size_it_shows() {
    let fake = FakeSafari::start().await;
    fake.screen(393, 852);
    let tools = Arc::new(FakeIosTools::with_xcode());
    let engine = engine(&fake, &tools);
    let view = engine
        .create_view(&sample_spec(EngineKind::Ios, 7))
        .await
        .expect("a view");
    let emulation = Emulation {
        width: 390,
        height: 844,
        ..Emulation::default()
    };
    let report = view.apply_emulation(&emulation).await.expect("a report");
    let FieldSupport::Unsupported(reason) = &report.fields[0].support else {
        panic!("the width is not emulable: {report:?}");
    };
    assert!(reason.contains("iPhone 16 (iOS 18.2)"), "{reason}");
    assert!(reason.contains("393x852"), "{reason}");
    assert!(fake.requests_to("/window/rect").is_empty());
}

#[tokio::test]
async fn a_click_reaches_the_page_as_a_touch_and_hover_sends_nothing() {
    let fake = phone_fake().await;
    let tools = Arc::new(FakeIosTools::with_xcode());
    let engine = engine(&fake, &tools);
    let view = engine
        .create_view(&sample_spec(EngineKind::Ios, 7))
        .await
        .expect("a view");
    let press = InputEvent::Mouse {
        phase: Phase::Down,
        x: 10.0,
        y: 20.0,
        button: Button::Left,
        clicks: 1,
        modifiers: 0,
    };
    view.dispatch_input(&press).await.expect("a touch");
    let hover = InputEvent::Mouse {
        phase: Phase::Move,
        x: 30.0,
        y: 40.0,
        button: Button::None,
        clicks: 0,
        modifiers: 0,
    };
    view.dispatch_input(&hover).await.expect("nothing to do");
    let actions = fake.actions();
    assert_eq!(actions.len(), 1, "{actions:?}");
    assert_eq!(actions[0][0]["parameters"]["pointerType"], "touch");
}

#[tokio::test]
async fn an_ios_session_leaves_the_desktop_safari_version_alone() {
    let fake = phone_fake().await;
    let tools = Arc::new(FakeIosTools::with_xcode());
    let engine = engine(&fake, &tools);
    let before = crate::platform::cached_version();
    engine
        .create_view(&sample_spec(EngineKind::Ios, 7))
        .await
        .expect("a view");
    assert_eq!(crate::platform::cached_version(), before);
}
