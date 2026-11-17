use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use base64::Engine as _;
use rb_engine::chromium::hidden::{RenderBackend, RenderSpec, WaitSpec};
use rb_engine::{Emulation, EngineError};
use serde_json::{Value, json};

use super::{CdpRenderBackend, MARKER_PREFIX, render_metrics, scripts};
use crate::cdp::{CdpRouter, CdpTransport, lock};

/// A browser that makes hidden targets, loads pages with their lifecycle events and answers
/// the render view's scripts.
#[derive(Default)]
struct Scripted {
    router: Mutex<Weak<CdpRouter>>,
    sent: Mutex<Vec<Value>>,
    refuse_hidden: bool,
    last_url: Mutex<String>,
}

impl Scripted {
    fn deliver(&self, message: Value) {
        if let Some(router) = lock(&self.router).upgrade() {
            router.on_message(message);
        }
    }

    fn sent(&self, method: &str) -> Vec<Value> {
        lock(&self.sent)
            .iter()
            .filter(|message| message["method"] == method)
            .cloned()
            .collect()
    }

    fn lifecycle(&self, session: &str, name: &str, loader: &str) {
        self.deliver(json!({
            "method": "Page.lifecycleEvent",
            "sessionId": session,
            "params": { "frameId": "F", "loaderId": loader, "name": name, "timestamp": 1 }
        }));
    }

    fn answer(&self, message: &Value) -> Value {
        let params = &message["params"];
        match message["method"].as_str().unwrap_or_default() {
            "Target.createTarget" if self.refuse_hidden && params["hidden"] == true => {
                json!({ "error": { "code": -32602, "message": "hidden is not supported" } })
            }
            "Target.createTarget" => json!({ "result": { "targetId": "R1" } }),
            "Target.getTargetInfo" => {
                json!({ "result": { "targetInfo": { "browserContextId": "CTX" } } })
            }
            "Target.attachToTarget" => json!({ "result": { "sessionId": "RS1" } }),
            "Page.navigate" => {
                let url = params["url"].as_str().unwrap_or_default();
                *lock(&self.last_url) = url.to_owned();
                if url.contains("broken") {
                    json!({ "result": { "frameId": "F", "errorText": "net::ERR_NAME_NOT_RESOLVED" } })
                } else {
                    json!({ "result": { "frameId": "F", "loaderId": "L1" } })
                }
            }
            "Runtime.evaluate" => {
                let expression = params["expression"].as_str().unwrap_or_default();
                let value = if expression == scripts::NAVIGATION {
                    json!({ "status": 404, "url": *lock(&self.last_url) })
                } else if expression.contains("__rbCaptureClean.prepare(") {
                    json!({ "token": 1 })
                } else if expression == scripts::READY_STATE {
                    json!("complete")
                } else {
                    json!(true)
                };
                json!({ "result": { "result": { "type": "object", "value": value } } })
            }
            "Page.getLayoutMetrics" => json!({ "result": {
                "cssVisualViewport": { "pageX": 0, "pageY": 0 },
                "cssLayoutViewport": { "pageX": 0, "pageY": 0 },
                "cssContentSize": { "height": 2000 }
            } }),
            "Page.getFrameTree" => json!({ "result": { "frameTree": { "frame": { "id": "F" } } } }),
            "Page.createIsolatedWorld" => json!({ "result": { "executionContextId": 9 } }),
            "Page.captureScreenshot" => json!({ "result": {
                "data": base64::engine::general_purpose::STANDARD.encode(b"PNG")
            } }),
            _ => json!({ "result": {} }),
        }
    }
}

impl CdpTransport for Scripted {
    fn send(&self, frame: Vec<u8>) -> Result<(), EngineError> {
        let body = frame.strip_suffix(&[0]).unwrap_or(&frame);
        let message: Value = serde_json::from_slice(body).expect("json");
        lock(&self.sent).push(message.clone());
        let mut reply = self.answer(&message);
        reply["id"] = message["id"].clone();
        self.deliver(reply);
        if message["method"] == "Page.navigate" && message["params"]["url"] != "about:blank" {
            // Another view's load first: it must not count.
            self.lifecycle("OTHER", "load", "L1");
            self.lifecycle("RS1", "DOMContentLoaded", "L1");
            self.lifecycle("RS1", "load", "L1");
            self.lifecycle("RS1", "networkIdle", "L1");
        }
        Ok(())
    }
}

fn browser(refuse_hidden: bool) -> (Arc<Scripted>, CdpRenderBackend) {
    let scripted = Arc::new(Scripted {
        refuse_hidden,
        ..Scripted::default()
    });
    let router = CdpRouter::new(scripted.clone());
    *lock(&scripted.router) = Arc::downgrade(&router);
    let backend = CdpRenderBackend::new(
        router,
        Box::new(|session| (session == "staging").then(|| "GRID-T2".to_owned())),
    );
    (scripted, backend)
}

fn spec(session: &str) -> RenderSpec {
    RenderSpec {
        breakpoint: "mobile".to_owned(),
        emulation: Emulation {
            width: 375,
            height: 667,
            dpr: 2.0,
            mobile: true,
            ..Emulation::default()
        },
        session: session.to_owned(),
    }
}

#[tokio::test]
async fn a_view_is_a_hidden_target_with_the_breakpoints_metrics() {
    let (scripted, backend) = browser(false);
    let view = backend.open(&spec("default")).await.expect("opens");
    let created = scripted.sent("Target.createTarget");
    assert_eq!(created.len(), 1);
    assert_eq!(created[0]["params"]["hidden"], true);
    assert!(created[0]["params"].get("browserContextId").is_none());
    let marker = created[0]["params"]["url"].as_str().expect("url");
    assert_eq!(marker, format!("{MARKER_PREFIX}{}", view.0));
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
    assert_eq!(scripted.sent("Target.closeTarget").len(), 1);
    assert!(!crate::devtools::hidden::is_hidden(
        "R1",
        "https://any.test/"
    ));
}

#[tokio::test]
async fn a_page_loads_waits_and_is_captured_full_page() {
    let (scripted, backend) = browser(false);
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

#[tokio::test]
async fn a_screenshot_runs_on_a_prepared_page_and_puts_it_back() {
    let (scripted, backend) = browser(false);
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
    let (_scripted, backend) = browser(false);
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

#[tokio::test]
async fn a_refused_hidden_target_falls_back_to_a_background_one() {
    let (scripted, backend) = browser(true);
    backend.open(&spec("default")).await.expect("opens");
    let created = scripted.sent("Target.createTarget");
    assert_eq!(created.len(), 2);
    assert!(created[1]["params"].get("hidden").is_none());
    assert_eq!(created[1]["params"]["background"], true);
}

#[tokio::test]
async fn a_named_session_takes_its_grid_views_context_and_an_unknown_one_is_refused() {
    let (scripted, backend) = browser(false);
    backend.open(&spec("staging")).await.expect("opens");
    assert_eq!(
        scripted.sent("Target.getTargetInfo")[0]["params"]["targetId"],
        "GRID-T2"
    );
    assert_eq!(
        scripted.sent("Target.createTarget")[0]["params"]["browserContextId"],
        "CTX"
    );
    let refused = backend.open(&spec("nobody")).await.expect_err("refused");
    assert!(refused.to_string().contains("nobody"), "{refused}");
    assert_eq!(scripted.sent("Target.createTarget").len(), 1);
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
