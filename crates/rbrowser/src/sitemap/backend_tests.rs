use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use rb_engine::Emulation;
use rb_engine::chromium::hidden::{RenderBackend, WaitSpec};
use serde_json::{Value, json};

use super::{CdpRenderBackend, MARKER_PREFIX, render_metrics, scripts};
use crate::cdp::lock;
use crate::sitemap::render_views::RenderView;

#[path = "backend_fake.rs"]
mod fake;

use fake::{Fixture, RESIZE, Scripted, Window, browser, browser_with, spec};

#[tokio::test]
async fn a_view_is_a_windowless_browser_found_by_its_marker_with_the_breakpoints_metrics() {
    let Fixture {
        scripted,
        window,
        backend,
        ..
    } = browser();
    let view = backend.open(&spec("default")).await.expect("opens");
    let marker = lock(&window.opened)[0].marker.clone();
    assert!(marker.starts_with(MARKER_PREFIX), "{marker}");
    assert_eq!(
        *lock(&window.opened),
        // At the breakpoint's pixel ratio, as its emulation asks for (RB-090-90).
        [RenderView::new(&marker, "default", 375, 667, 2.0)]
    );
    // No hidden target any more: the window made the browser, the backend found it.
    assert!(scripted.sent("Target.createTarget").is_empty());
    assert_eq!(
        scripted.sent("Target.attachToTarget")[0]["params"]["targetId"],
        "R1"
    );
    assert!(crate::devtools::hidden::is_hidden(
        "R1",
        "https://any.test/"
    ));
    let metrics = scripted.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(metrics[0]["sessionId"], "RS1");
    assert_eq!(metrics[0]["params"]["width"], 375);
    assert_eq!(metrics[0]["params"]["height"], 667);
    assert_eq!(metrics[0]["params"]["scale"], 1.0);
    assert_eq!(
        scripted.sent("Page.setLifecycleEventsEnabled")[0]["sessionId"],
        "RS1"
    );
    assert!(
        !scripted
            .sent("Emulation.setTouchEmulationEnabled")
            .is_empty()
    );
    backend.close(view).await;
    assert_eq!(*lock(&window.closed), [marker]);
    assert!(scripted.sent("Target.closeTarget").is_empty());
    assert!(!crate::devtools::hidden::is_hidden(
        "R1",
        "https://any.test/"
    ));
}

#[tokio::test(start_paused = true)]
async fn the_marker_is_looked_for_until_the_browser_lists_it() {
    let Fixture {
        scripted, backend, ..
    } = browser_with(
        Scripted {
            hide_first: AtomicUsize::new(3),
            ..Scripted::default()
        },
        Window::default(),
    );
    backend.open(&spec("default")).await.expect("opens");
    assert_eq!(scripted.sent("Target.getTargets").len(), 4);
    assert_eq!(
        scripted.sent("Target.attachToTarget")[0]["params"]["targetId"],
        "R1"
    );
}

#[tokio::test(start_paused = true)]
async fn a_browser_that_never_lists_its_marker_fails_the_open_and_is_closed() {
    let Fixture {
        scripted,
        window,
        backend,
        ..
    } = browser_with(
        Scripted::default(),
        Window {
            lost: true,
            ..Window::default()
        },
    );
    let error = backend
        .open(&spec("default"))
        .await
        .expect_err("never found");
    assert!(error.to_string().contains("no CDP target shows"), "{error}");
    let marker = lock(&window.opened)[0].marker.clone();
    assert_eq!(*lock(&window.closed), [marker]);
    assert!(scripted.sent("Target.attachToTarget").is_empty());
}

#[tokio::test]
async fn two_backends_never_share_a_marker() {
    // A sitemap capture and an environment comparison at once: each has a backend of its own.
    let Fixture {
        scripted,
        window,
        router,
        backend,
    } = browser();
    let other = CdpRenderBackend::new(router, Box::new(|_: &str| None), window.clone());
    let first = backend.open(&spec("default")).await.expect("opens");
    let second = other.open(&spec("default")).await.expect("opens");
    let markers: Vec<String> = lock(&window.opened)
        .iter()
        .map(|view| view.marker.clone())
        .collect();
    assert_eq!(markers.len(), 2);
    assert_ne!(markers[0], markers[1]);
    let attached: Vec<Value> = scripted
        .sent("Target.attachToTarget")
        .iter()
        .map(|message| message["params"]["targetId"].clone())
        .collect();
    assert_eq!(attached, [json!("R1"), json!("R2")]);
    // Closing one leaves the other's browser alone.
    backend.close(first).await;
    assert_eq!(*lock(&window.closed), [markers[0].clone()]);
    other.close(second).await;
    assert_eq!(*lock(&window.closed), markers);
}

#[tokio::test]
async fn a_grid_popup_or_another_context_at_the_marker_is_never_attached() {
    let Fixture {
        scripted, backend, ..
    } = browser_with(
        Scripted::default(),
        Window {
            decoys: true,
            ..Window::default()
        },
    );
    backend.open(&spec("default")).await.expect("opens");
    let attached: Vec<Value> = scripted
        .sent("Target.attachToTarget")
        .iter()
        .map(|message| message["params"]["targetId"].clone())
        .collect();
    assert_eq!(attached, [json!("R1")]);
    assert!(!crate::devtools::hidden::is_hidden(
        "P1",
        "https://any.test/"
    ));
    // The context to hold the target to is the session's grid view's.
    assert_eq!(
        scripted.sent("Target.getTargetInfo")[0]["params"]["targetId"],
        "GRID-T1"
    );
}

#[tokio::test]
async fn a_window_that_cannot_make_the_browser_fails_the_open() {
    let Fixture {
        scripted, backend, ..
    } = browser_with(
        Scripted::default(),
        Window {
            refuse: true,
            ..Window::default()
        },
    );
    let error = backend.open(&spec("default")).await.expect_err("refused");
    assert!(
        error.to_string().contains("no window to make it in"),
        "{error}"
    );
    assert!(scripted.sent("Target.getTargets").is_empty());
}

#[tokio::test]
async fn a_named_session_needs_a_grid_view_showing_it() {
    let Fixture {
        window, backend, ..
    } = browser();
    backend.open(&spec("staging")).await.expect("opens");
    assert_eq!(lock(&window.opened)[0].session, "staging");
    let refused = backend.open(&spec("nobody")).await.expect_err("refused");
    assert!(refused.to_string().contains("nobody"), "{refused}");
    assert_eq!(lock(&window.opened).len(), 1, "no browser for it");
}

#[tokio::test]
async fn a_page_loads_waits_and_is_captured_full_page() {
    let Fixture {
        scripted, backend, ..
    } = browser();
    let view = backend.open(&spec("default")).await.expect("opens");
    let loaded = backend
        .navigate(view, "http://127.0.0.1:9/missing")
        .await
        .expect("loads");
    assert_eq!(loaded.status, Some(404));
    assert_eq!(loaded.final_url, None);
    let wait = WaitSpec {
        selector: Some("main".to_owned()),
        delay: Duration::from_millis(1),
        ..WaitSpec::default()
    };
    tokio::time::timeout(Duration::from_secs(5), backend.wait(view, &wait))
        .await
        .expect("network idle was seen during the load")
        .expect("waits");
    let scripts_run: Vec<String> = scripted
        .sent("Runtime.evaluate")
        .iter()
        .filter_map(|message| message["params"]["expression"].as_str().map(str::to_owned))
        .collect();
    assert!(scripts_run.contains(&scripts::has_selector("main")));
    assert!(scripts_run.contains(&scripts::AUTO_SCROLL.to_owned()));
    let png = backend.screenshot(view, true).await.expect("captures");
    assert_eq!(png, b"PNG");
    let shot = &scripted.sent("Page.captureScreenshot")[0];
    assert_eq!(shot["params"]["captureBeyondViewport"], true);
    assert_eq!(shot["sessionId"], "RS1");
    // The metrics go back to the breakpoint's after the full-page height.
    let metrics = scripted.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(metrics.last().expect("restored")["params"]["height"], 667);
    // Without a plan the page is left as it is.
    assert!(scripted.sent("Page.createIsolatedWorld").is_empty());
    assert!(scripted.sent("Animation.setPlaybackRate").is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_screenshot_without_a_frame_fails_at_the_shots_own_deadline() {
    let Fixture { backend, .. } = browser_with(
        Scripted {
            stall_shots: true,
            ..Scripted::default()
        },
        Window::default(),
    );
    let backend = backend.with_shot_timeout(Duration::from_secs(15));
    let view = backend.open(&spec("default")).await.expect("opens");
    let started = tokio::time::Instant::now();
    let error = backend.screenshot(view, false).await.expect_err("no frame");
    assert!(
        error.to_string().contains("no frame within 15 s"),
        "{error}"
    );
    // The shot's own deadline, not the router's 30 s ("no answer within 30 s").
    assert!(started.elapsed() >= Duration::from_secs(15));
}

#[tokio::test(start_paused = true)]
async fn a_full_page_shot_given_up_puts_the_breakpoints_size_back() {
    let Fixture {
        scripted,
        window,
        backend,
        ..
    } = browser_with(
        Scripted {
            stall_shots: true,
            ..Scripted::default()
        },
        Window::default(),
    );
    let backend = backend.with_shot_timeout(Duration::from_secs(15));
    let view = backend.open(&spec("default")).await.expect("opens");
    backend
        .navigate(view, "http://127.0.0.1:9/long")
        .await
        .expect("loads");
    backend.screenshot(view, true).await.expect_err("no frame");
    // The capture grew the view to the page's height and was dropped: the size goes back.
    let metrics = scripted.sent("Emulation.setDeviceMetricsOverride");
    // The full-page override sends the height as a float (`capture::cdp::with_height`).
    assert!(
        metrics
            .iter()
            .any(|call| call["params"]["height"] == 2000.0)
    );
    assert_eq!(metrics.last().expect("restored")["params"]["height"], 667);
    // The windowless view grew with it and is back at the breakpoint's size too (RB-090-88).
    let marker = lock(&window.opened)[0].marker.clone();
    assert_eq!(
        *lock(&window.resized),
        [(marker.clone(), 375, 2000), (marker, 375, 667)]
    );
    // And every page starts at the breakpoint's size, whatever the shot before left.
    backend
        .navigate(view, "http://127.0.0.1:9/next")
        .await
        .expect("loads");
    let order: Vec<Value> = lock(&scripted.sent).clone();
    let navigate = order
        .iter()
        .rposition(|message| message["method"] == "Page.navigate")
        .expect("navigated");
    let before = &order[navigate - 1];
    assert_eq!(before["method"], "Emulation.setDeviceMetricsOverride");
    assert_eq!(before["params"]["height"], 667);
}

#[tokio::test]
async fn a_screenshot_runs_on_a_prepared_page_and_puts_it_back() {
    let Fixture {
        scripted, backend, ..
    } = browser();
    let plan = rb_visual::CleanSettings {
        hide_selectors: vec!["#banner".to_owned()],
        settle_ms: 0,
        ..rb_visual::CleanSettings::default()
    }
    .plan(&[], rb_visual::CleanOverrides::default());
    let backend = backend.with_clean(plan);
    let view = backend.open(&spec("default")).await.expect("opens");
    backend
        .navigate(view, "http://127.0.0.1:9/page")
        .await
        .expect("loads");
    backend.screenshot(view, false).await.expect("captures");
    let order: Vec<String> = lock(&scripted.sent)
        .iter()
        .filter_map(|message| message["method"].as_str().map(str::to_owned))
        .collect();
    let at = |name: &str| order.iter().position(|method| method == name);
    let last = |name: &str| order.iter().rposition(|method| method == name);
    assert!(at("Page.createIsolatedWorld") < at("Page.captureScreenshot"));
    assert!(at("Page.captureScreenshot") < last("Animation.setPlaybackRate"));
    let evaluations = scripted.sent("Runtime.evaluate");
    let clean: Vec<&Value> = evaluations
        .iter()
        .filter(|message| message["params"]["contextId"] == 9)
        .collect();
    assert_eq!(clean.len(), 2, "prepare and undo in the isolated world");
    assert!(
        clean[0]["params"]["expression"]
            .as_str()
            .is_some_and(|expression| expression.contains("#banner"))
    );
    assert!(
        clean[1]["params"]["expression"]
            .as_str()
            .is_some_and(|expression| expression.ends_with("undo(1)"))
    );
    let media = scripted.sent("Emulation.setEmulatedMedia");
    assert_eq!(
        media.last().expect("put back")["params"],
        crate::emulation::cdp::media_params(&spec("default").emulation)
    );
}

#[tokio::test]
async fn a_navigation_error_fails_the_load() {
    let Fixture { backend, .. } = browser();
    let view = backend.open(&spec("default")).await.expect("opens");
    let error = backend
        .navigate(view, "http://broken.test/")
        .await
        .expect_err("fails");
    assert!(
        error.to_string().contains("ERR_NAME_NOT_RESOLVED"),
        "{error}"
    );
}

#[test]
fn render_metrics_are_the_breakpoints_own_size_at_scale_one() {
    let metrics = render_metrics(&Emulation {
        width: 1280,
        height: 800,
        dpr: 0.0,
        ..Emulation::default()
    });
    assert_eq!(metrics["width"], 1280);
    assert_eq!(metrics["deviceScaleFactor"], 0.0);
    assert_eq!(metrics["scale"], 1.0);
}

/// The methods sent from the last `Page.getLayoutMetrics` on, the window's resizes among them.
fn from_the_shot(scripted: &Scripted) -> Vec<(String, Value)> {
    let sent = lock(&scripted.sent).clone();
    let start = sent
        .iter()
        .rposition(|message| message["method"] == "Page.getLayoutMetrics")
        .expect("a shot");
    sent[start..]
        .iter()
        .filter(|message| message["method"] != "Runtime.evaluate")
        .map(|message| {
            let method = message["method"].as_str().unwrap_or_default().to_owned();
            let height = message["params"]["height"].clone();
            (method, height)
        })
        .collect()
}

#[tokio::test]
async fn a_full_page_shot_grows_the_windowless_view_to_the_page_and_back() {
    // A windowless view paints only its own rectangle: the page's height in the emulation alone
    // repeated the top viewport down the image (RB-090-88). It grows after the override, the shot
    // waits for its frame, and it shrinks after the restore.
    let Fixture {
        scripted,
        window,
        backend,
        ..
    } = browser();
    let view = backend.open(&spec("default")).await.expect("opens");
    backend
        .navigate(view, "http://127.0.0.1:9/long")
        .await
        .expect("loads");
    assert_eq!(
        backend.screenshot(view, true).await.expect("captures"),
        b"PNG"
    );
    let order = from_the_shot(&scripted);
    let expected: Vec<(String, Value)> = vec![
        ("Page.getLayoutMetrics".to_owned(), Value::Null),
        (
            "Emulation.setDeviceMetricsOverride".to_owned(),
            json!(2000.0),
        ),
        (RESIZE.to_owned(), json!(2000)),
        ("Page.captureScreenshot".to_owned(), Value::Null),
        ("Emulation.setDeviceMetricsOverride".to_owned(), json!(667)),
        (RESIZE.to_owned(), json!(667)),
    ];
    assert_eq!(order, expected);
    // At the breakpoint's width, the view it was made for.
    let marker = lock(&window.opened)[0].marker.clone();
    assert_eq!(lock(&window.resized)[0], (marker, 375, 2000));
}

#[tokio::test]
async fn a_viewport_shot_keeps_the_windowless_views_size() {
    let Fixture {
        window, backend, ..
    } = browser();
    let view = backend.open(&spec("default")).await.expect("opens");
    backend.screenshot(view, false).await.expect("captures");
    assert!(lock(&window.resized).is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_view_that_paints_no_frame_of_the_page_fails_the_shot() {
    // Never a frame of the old size saved as a full page (code review, wave 090-w23): the view is
    // asked once more, then the shot fails with its reason - into run.json like any failed shot -
    // and the view gets its breakpoint's size back.
    let Fixture {
        scripted,
        window,
        backend,
        ..
    } = browser_with(
        Scripted::default(),
        Window {
            no_frame: true,
            ..Window::default()
        },
    );
    let view = backend.open(&spec("default")).await.expect("opens");
    let started = tokio::time::Instant::now();
    let error = backend.screenshot(view, true).await.expect_err("no frame");
    assert!(
        error.to_string().contains("no frame at the page's height"),
        "{error}"
    );
    assert!(started.elapsed() >= crate::sitemap::render_surface::FRAME_WAIT * 2);
    assert!(scripted.sent("Page.captureScreenshot").is_empty());
    let heights: Vec<u32> = lock(&window.resized)
        .iter()
        .map(|(_, _, height)| *height)
        .collect();
    assert_eq!(heights, [2000, 2000, 667]);
    let metrics = scripted.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(metrics.last().expect("restored")["params"]["height"], 667);
}
