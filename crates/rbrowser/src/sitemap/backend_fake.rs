//! The fake browser and window of `backend_tests.rs`: a browser that lists the render views
//! the window made, loads pages with their lifecycle events and answers the render view's
//! scripts.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

use base64::Engine as _;
use rb_engine::chromium::hidden::RenderSpec;
use rb_engine::{Emulation, EngineError};
use serde_json::{Value, json};

use crate::cdp::{CdpRouter, CdpTransport, lock};
use crate::sitemap::backend::CdpRenderBackend;
use crate::sitemap::render_views::{RenderView, RenderViewMade, RenderViews};
use crate::sitemap::scripts;

/// A target the fake browser lists.
pub(super) struct Listed {
    pub(super) id: String,
    pub(super) url: String,
    pub(super) context: String,
    /// The page that opened it (a popup).
    pub(super) opener: Option<String>,
}

/// The browser context of each grid view's target.
fn context_of(target: &str) -> &'static str {
    match target {
        "GRID-T1" => "CTX-default",
        "GRID-T2" => "CTX-staging",
        _ => "CTX-unknown",
    }
}

/// A browser that lists the render views the window made, loads pages with their lifecycle
/// events and answers the render view's scripts.
#[derive(Default)]
pub(super) struct Scripted {
    pub(super) router: Mutex<Weak<CdpRouter>>,
    pub(super) sent: Mutex<Vec<Value>>,
    pub(super) last_url: Mutex<String>,
    /// What `Target.getTargets` lists besides a grid view.
    pub(super) targets: Mutex<Vec<Listed>>,
    /// How many `Target.getTargets` answers leave the render views out first (the browser has
    /// not committed its marker yet).
    pub(super) hide_first: AtomicUsize,
    /// `Page.captureScreenshot` never answers (a view without frames).
    pub(super) stall_shots: bool,
}

impl Scripted {
    fn deliver(&self, message: Value) {
        if let Some(router) = lock(&self.router).upgrade() {
            router.on_message(message);
        }
    }

    pub(super) fn sent(&self, method: &str) -> Vec<Value> {
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

    fn target_infos(&self) -> Value {
        let mut infos = vec![json!({
            "targetId": "GRID-T1", "type": "page", "url": "https://grid.test/",
            "browserContextId": "CTX-default"
        })];
        let hidden = self
            .hide_first
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok();
        if !hidden {
            infos.extend(lock(&self.targets).iter().map(|listed| {
                let mut info = json!({
                    "targetId": listed.id, "type": "page", "url": listed.url,
                    "browserContextId": listed.context
                });
                if let Some(opener) = &listed.opener {
                    info["openerId"] = json!(opener);
                }
                info
            }));
        }
        json!({ "result": { "targetInfos": infos } })
    }

    fn answer(&self, message: &Value) -> Value {
        let params = &message["params"];
        match message["method"].as_str().unwrap_or_default() {
            "Target.getTargets" => self.target_infos(),
            "Target.getTargetInfo" => {
                let target = params["targetId"].as_str().unwrap_or_default();
                json!({ "result": { "targetInfo": {
                    "targetId": target, "browserContextId": context_of(target)
                } } })
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
        if self.stall_shots && message["method"] == "Page.captureScreenshot" {
            return Ok(());
        }
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

/// The window: makes a render view's browser, which the browser then lists at its marker.
#[derive(Default)]
pub(super) struct Window {
    pub(super) browser: Mutex<Weak<Scripted>>,
    pub(super) opened: Mutex<Vec<RenderView>>,
    pub(super) closed: Mutex<Vec<String>>,
    /// The window cannot make a browser.
    pub(super) refuse: bool,
    /// The browser is made but never lists its marker.
    pub(super) lost: bool,
    /// A grid page's popup and a page of another context show the marker first.
    pub(super) decoys: bool,
}

impl RenderViews for Window {
    fn open(&self, view: RenderView, made: RenderViewMade) {
        lock(&self.opened).push(view.clone());
        if self.refuse {
            made(Err("no window to make it in".to_owned()));
            return;
        }
        if !self.lost
            && let Some(browser) = lock(&self.browser).upgrade()
        {
            let context = format!("CTX-{}", view.session);
            let mut targets = lock(&browser.targets);
            if self.decoys {
                targets.push(Listed {
                    id: "P1".to_owned(),
                    url: view.marker.clone(),
                    context: context.clone(),
                    opener: Some("GRID-T1".to_owned()),
                });
                targets.push(Listed {
                    id: "X1".to_owned(),
                    url: view.marker.clone(),
                    context: "CTX-other".to_owned(),
                    opener: None,
                });
            }
            let made = targets
                .iter()
                .filter(|listed| listed.id.starts_with('R'))
                .count();
            targets.push(Listed {
                id: format!("R{}", made + 1),
                url: view.marker,
                context,
                opener: None,
            });
        }
        made(Ok(()));
    }

    fn close(&self, marker: String) {
        if let Some(browser) = lock(&self.browser).upgrade() {
            lock(&browser.targets).retain(|listed| listed.url != marker);
        }
        lock(&self.closed).push(marker);
    }
}

pub(super) struct Fixture {
    pub(super) scripted: Arc<Scripted>,
    pub(super) window: Arc<Window>,
    pub(super) router: Arc<CdpRouter>,
    pub(super) backend: CdpRenderBackend,
}

pub(super) fn browser_with(scripted: Scripted, window: Window) -> Fixture {
    let scripted = Arc::new(scripted);
    let window = Arc::new(window);
    *lock(&window.browser) = Arc::downgrade(&scripted);
    let router = CdpRouter::new(scripted.clone());
    *lock(&scripted.router) = Arc::downgrade(&router);
    let backend = CdpRenderBackend::new(
        router.clone(),
        Box::new(|session| grid_target(session).map(str::to_owned)),
        window.clone(),
    );
    Fixture {
        scripted,
        window,
        router,
        backend,
    }
}

/// The grid view's target showing `session`: one for `default`, one for `staging`.
pub(super) fn grid_target(session: &str) -> Option<&'static str> {
    match session {
        "default" => Some("GRID-T1"),
        "staging" => Some("GRID-T2"),
        _ => None,
    }
}

pub(super) fn browser() -> Fixture {
    browser_with(Scripted::default(), Window::default())
}

pub(super) fn spec(session: &str) -> RenderSpec {
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
