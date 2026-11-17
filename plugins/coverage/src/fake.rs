//! A host for the native tests that plays Chromium's side: per view its style sheets (CDP id,
//! URL or `<style>`, text) and the rules a recalculation marks used, its scripts' V8 coverage
//! before and after a reload, the page's answers to the plugin's scripts - and a record of every
//! CDP method sent, what was written and reported, and whether tracking is left running.

use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::{Value, json};

use crate::page;

/// One style sheet of a fake page.
#[derive(Clone, Debug)]
pub struct Sheet {
    pub id: &'static str,
    /// None for a `<style>` element.
    pub href: Option<&'static str>,
    pub text: String,
    /// The rules marked used, `(start, end)`.
    pub used: Vec<(u32, u32)>,
}

/// One view of the fake browser.
#[derive(Clone, Debug, Default)]
pub struct FakeView {
    pub info: ViewInfo,
    pub sheets: Vec<Sheet>,
    /// `Profiler.takePreciseCoverage`'s `result` before and after the reload.
    pub scripts_before: Value,
    pub scripts_after: Value,
    pub css_tracking: bool,
    pub precise: bool,
    pub profiler_enabled: bool,
    pub reloaded: bool,
}

/// 2026-11-12 10:00:00 UTC.
pub const NOW: u64 = 1_794_477_600;
pub const PAGE: &str = "http://localhost:8080/coverage/index.html";

#[derive(Default)]
pub struct FakeHost {
    pub views: Vec<FakeView>,
    pub settings: BTreeMap<String, String>,
    /// Below `.rbrowser/out/coverage/`.
    pub written: BTreeMap<String, Vec<u8>>,
    pub reported: Vec<Finding>,
    /// Every CDP command, as `(view, method)`.
    pub sent: Vec<(ViewId, String)>,
    /// `(view, method)` pairs that fail.
    pub failing: BTreeSet<(ViewId, String)>,
    pub logs: Vec<(String, String)>,
    pub now: u64,
}

impl FakeHost {
    pub fn new() -> Self {
        Self {
            now: NOW,
            ..Self::default()
        }
    }

    /// Adds a Chromium view `(id, breakpoint, width)` on [`PAGE`].
    pub fn view(mut self, id: ViewId, breakpoint: &str, width: u32, sheets: Vec<Sheet>) -> Self {
        self.views.push(FakeView {
            info: ViewInfo {
                id,
                label: format!("View {id}"),
                breakpoint: breakpoint.to_owned(),
                engine: "chromium".to_owned(),
                width,
                height: 800,
                dpr: 1.0,
                url: PAGE.to_owned(),
                title: "coverage".to_owned(),
            },
            sheets,
            scripts_before: json!([]),
            scripts_after: json!([]),
            ..FakeView::default()
        });
        self
    }

    pub fn engine(mut self, id: ViewId, engine: &str) -> Self {
        if let Some(view) = self.views.iter_mut().find(|view| view.info.id == id) {
            view.info.engine = engine.to_owned();
        }
        self
    }

    pub fn scripts(mut self, id: ViewId, before: Value, after: Value) -> Self {
        if let Some(view) = self.views.iter_mut().find(|view| view.info.id == id) {
            view.scripts_before = before;
            view.scripts_after = after;
        }
        self
    }

    pub fn set(mut self, key: &str, value: &str) -> Self {
        self.settings.insert(key.to_owned(), value.to_owned());
        self
    }

    pub fn fail(mut self, id: ViewId, method: &str) -> Self {
        self.failing.insert((id, method.to_owned()));
        self
    }

    /// The methods sent to `view`, in order.
    pub fn methods(&self, view: ViewId) -> Vec<&str> {
        self.sent
            .iter()
            .filter(|(id, _)| *id == view)
            .map(|(_, method)| method.as_str())
            .collect()
    }

    /// Whether any view still tracks CSS rules or precise coverage.
    pub fn tracking(&self) -> bool {
        self.views
            .iter()
            .any(|view| view.css_tracking || view.precise)
    }

    fn view_mut(&mut self, id: ViewId) -> Result<&mut FakeView, Failure> {
        let view = self
            .views
            .iter_mut()
            .find(|view| view.info.id == id)
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))?;
        if view.info.engine != "chromium" {
            return Err(Failure::coded(
                FailureKind::NotSupported,
                codes::NOT_AVAILABLE,
            ));
        }
        Ok(view)
    }

    fn usage(view: &FakeView) -> Value {
        let mut usage = Vec::new();
        for sheet in &view.sheets {
            for (start, end) in &sheet.used {
                usage.push(json!({ "styleSheetId": sheet.id, "startOffset": start, "endOffset": end, "used": true }));
            }
        }
        Value::Array(usage)
    }
}

fn failed(method: &str) -> Failure {
    Failure::coded(FailureKind::Other, codes::HOST_FAILED).with_param("reason", method)
}

impl PluginHost for FakeHost {
    fn log(&mut self, level: &str, message: &str) {
        self.logs.push((level.to_owned(), message.to_owned()));
    }

    fn now_unix_seconds(&mut self) -> u64 {
        self.now
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Ok(self.views.iter().map(|view| view.info.clone()).collect())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(self.views.first().map(|view| view.info.id))
    }

    fn evaluate(&mut self, id: ViewId, script: &str) -> Result<String, Failure> {
        let view = self.view_mut(id)?;
        let answer = if script == page::SHEETS {
            let sheets: Vec<Value> = view
                .sheets
                .iter()
                .map(|sheet| match sheet.href {
                    Some(href) => json!({ "href": href }),
                    None => json!({ "text": sheet.text }),
                })
                .collect();
            json!({ "url": PAGE, "sheets": sheets })
        } else if script == page::RELOAD {
            view.reloaded = true;
            json!(1000.5)
        } else if script.contains("performance.timeOrigin === 1000.5") {
            if view.reloaded {
                json!({ "state": "ready", "url": PAGE })
            } else {
                json!({ "state": "old" })
            }
        } else {
            Value::Null
        };
        Ok(answer.to_string())
    }

    fn cdp_send(&mut self, id: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        self.sent.push((id, method.to_owned()));
        let failing = self.failing.contains(&(id, method.to_owned()));
        let params: Value = serde_json::from_str(params).expect("the plugin sends JSON");
        let view = self.view_mut(id)?;
        if failing {
            return Err(failed(method));
        }
        let answer = match method {
            "DOM.enable" | "CSS.enable" => json!({}),
            "Profiler.enable" => {
                view.profiler_enabled = true;
                json!({})
            }
            "CSS.startRuleUsageTracking" => {
                view.css_tracking = true;
                json!({})
            }
            "CSS.takeCoverageDelta" if view.css_tracking => {
                json!({ "coverage": Self::usage(view), "timestamp": 1.0 })
            }
            "CSS.stopRuleUsageTracking" if view.css_tracking => {
                view.css_tracking = false;
                json!({ "ruleUsage": Self::usage(view) })
            }
            "CSS.getStyleSheetText" => {
                let id = params["styleSheetId"].as_str().unwrap_or_default();
                let sheet = view.sheets.iter().find(|sheet| sheet.id == id);
                return sheet
                    .map(|sheet| json!({ "text": sheet.text }).to_string())
                    .ok_or_else(|| failed(method));
            }
            "Profiler.startPreciseCoverage" if view.profiler_enabled => {
                view.precise = true;
                json!({ "timestamp": 1.0 })
            }
            "Profiler.takePreciseCoverage" if view.precise => {
                let result = if view.reloaded {
                    view.scripts_after.clone()
                } else {
                    view.scripts_before.clone()
                };
                json!({ "result": result, "timestamp": 2.0 })
            }
            "Profiler.stopPreciseCoverage" => {
                view.precise = false;
                json!({})
            }
            "Page.getFrameTree" => json!({ "frameTree": { "frame": { "id": "F1", "url": PAGE } } }),
            "Page.getResourceContent" => {
                assert_eq!(params["frameId"], "F1");
                let url = params["url"].as_str().unwrap_or_default();
                let sheet = view.sheets.iter().find(|sheet| sheet.href == Some(url));
                return sheet
                    .map(|sheet| {
                        json!({ "content": sheet.text, "base64Encoded": false }).to_string()
                    })
                    .ok_or_else(|| failed(method));
            }
            _ => return Err(failed(method)),
        };
        Ok(answer.to_string())
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        path.strip_prefix(".rbrowser/out/coverage/")
            .and_then(|rest| self.written.get(rest))
            .cloned()
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Ok(Vec::new())
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.written.insert(path.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        self.reported.extend_from_slice(items);
        Ok(())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
