//! The checks every [`Engine`] must pass, whatever it drives: [`check_engine`] opens a view and
//! walks it through the contract - navigation with its event, history, emulation with the
//! engine's published report, input, screenshot, the window operations (external engines only)
//! and close, which ends an external view's events. [`check_unavailable`] is the contract of an
//! engine that cannot run. Both panic with the broken rule.

use std::time::Duration;

use rb_config::{Breakpoint, EmulationFields, EngineKind};

use crate::emulation_report::EmulationReport;
use crate::engine::{
    Button, Engine, EngineHealth, HandleEvent, HandleEvents, InputEvent, Phase, ScreenshotRequest,
    ViewSpec,
};
use crate::external::WindowRect;
use crate::{Emulation, EngineError, ViewId};

/// How long an event may take.
const EVENT_WAIT: Duration = Duration::from_secs(2);

/// A spec for `engine` with id `view`: a phone breakpoint whose emulation asks for touch,
/// reduced motion and a mobile device, so engines differ in their reports.
pub fn sample_spec(engine: EngineKind, view: u32) -> ViewSpec {
    let breakpoint = Breakpoint {
        id: format!("{engine:?}-phone").to_lowercase(),
        label: Some("Phone".to_owned()),
        width: Some(390),
        height: Some(844),
        device: None,
        engine,
        ios_device: None,
        session: None,
        sync_navigation: true,
        shortcut: None,
        color: None,
        emulation: EmulationFields::default(),
    };
    let emulation = Emulation {
        width: 390,
        height: 844,
        dpr: 3.0,
        mobile: true,
        touch: true,
        reduced_motion: Some(true),
        ..Emulation::default()
    };
    ViewSpec {
        view: Some(ViewId(view)),
        breakpoint,
        emulation,
        url: "http://localhost/start".to_owned(),
        title: "rBrowser \u{b7} Phone".to_owned(),
        window: Some(WindowRect {
            x: 10,
            y: 20,
            width: 390,
            height: 844,
        }),
        cell: None,
    }
}

/// Walks one view of `engine`, made from `spec`, through the contract.
pub async fn check_engine(engine: &dyn Engine, spec: &ViewSpec) {
    let kind = engine.kind();
    let handle = match engine.create_view(spec).await {
        Ok(handle) => handle,
        Err(error) => panic!("{kind:?}: create_view failed: {error}"),
    };
    assert_eq!(handle.engine(), kind, "the handle names its engine");
    assert_eq!(
        handle.is_external(),
        engine.external(),
        "{kind:?}: handle and engine agree on external"
    );
    if engine.external() {
        assert_eq!(
            Some(handle.id()),
            spec.view,
            "{kind:?}: an external view keeps its cell's id"
        );
    }
    assert_eq!(
        engine.health(),
        EngineHealth::Running,
        "{kind:?}: running after its first view"
    );

    let mut events = handle.events();
    let url = "http://localhost/conformance";
    ok(kind, "navigate", handle.navigate(url).await);
    assert_eq!(
        next_navigation(&mut events).await,
        Some(url.to_owned()),
        "{kind:?}: navigate announces the committed URL"
    );
    ok(kind, "reload", handle.reload(false).await);
    ok(kind, "back", handle.back().await);
    ok(kind, "forward", handle.forward().await);

    let report = handle.apply_emulation(&spec.emulation).await;
    let report = report.unwrap_or_else(|error| panic!("{kind:?}: apply_emulation: {error}"));
    assert_eq!(
        report,
        EmulationReport::for_engine(kind, &spec.emulation),
        "{kind:?}: the report is the engine's published table"
    );

    for phase in [Phase::Down, Phase::Up] {
        let click = InputEvent::Mouse {
            phase,
            x: 10.0,
            y: 20.0,
            button: Button::Left,
            clicks: 1,
            modifiers: 0,
        };
        ok(kind, "dispatch_input", handle.dispatch_input(&click).await);
    }

    let shot = handle.screenshot(ScreenshotRequest::default()).await;
    let shot = shot.unwrap_or_else(|error| panic!("{kind:?}: screenshot: {error}"));
    assert!(
        !shot.png_base64.is_empty(),
        "{kind:?}: a screenshot has data"
    );

    let rect = WindowRect {
        x: 50,
        y: 60,
        width: 400,
        height: 700,
    };
    if engine.external() {
        ok(kind, "set_window", handle.set_window(rect).await);
        ok(kind, "raise", handle.raise().await);
    } else {
        assert!(
            matches!(
                handle.set_window(rect).await,
                Err(EngineError::Unsupported { .. })
            ),
            "{kind:?}: a grid view has no window to place"
        );
        assert!(
            matches!(handle.raise().await, Err(EngineError::Unsupported { .. })),
            "{kind:?}: a grid view has no window to raise"
        );
    }

    let mut ending = handle.events();
    ok(kind, "close", handle.close().await);
    if engine.external() {
        assert!(
            ends(&mut ending).await,
            "{kind:?}: close ends the view's events (Exited or their end)"
        );
    }
    ok(kind, "close twice", handle.close().await);
    assert!(
        matches!(handle.navigate(url).await, Err(EngineError::Closed)),
        "{kind:?}: a closed view refuses work"
    );
}

/// The contract of an engine that cannot run: unavailable with a reason, and no view.
pub async fn check_unavailable(engine: &dyn Engine, spec: &ViewSpec) {
    let kind = engine.kind();
    let EngineHealth::Unavailable(reason) = engine.health() else {
        panic!("{kind:?}: an unavailable engine says so");
    };
    assert!(!reason.is_empty(), "{kind:?}: and says why");
    assert!(
        matches!(
            engine.create_view(spec).await,
            Err(EngineError::Unavailable { engine: named, .. }) if named == kind
        ),
        "{kind:?}: opens no view"
    );
    engine.shutdown().await;
}

fn ok(kind: EngineKind, what: &str, result: Result<(), EngineError>) {
    if let Err(error) = result {
        panic!("{kind:?}: {what} failed: {error}");
    }
}

/// Whether `events` end (`Exited` or their end) within the wait, skipping the rest.
async fn ends(events: &mut HandleEvents) -> bool {
    let wait = async {
        loop {
            match events.recv().await {
                Some(HandleEvent::Exited { .. }) | None => return,
                Some(
                    HandleEvent::Navigated { .. }
                    | HandleEvent::Updated
                    | HandleEvent::WindowReplaced,
                ) => {}
            }
        }
    };
    tokio::time::timeout(EVENT_WAIT, wait).await.is_ok()
}

/// The URL of the next navigation event, skipping updates.
async fn next_navigation(events: &mut HandleEvents) -> Option<String> {
    let wait = async {
        loop {
            match events.recv().await {
                Some(HandleEvent::Navigated { url }) => return Some(url),
                Some(HandleEvent::Updated | HandleEvent::WindowReplaced) => {}
                Some(HandleEvent::Exited { .. }) | None => return None,
            }
        }
    };
    tokio::time::timeout(EVENT_WAIT, wait).await.ok().flatten()
}
