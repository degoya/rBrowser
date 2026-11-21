//! A host for the native tests: Chromium views, the plugin's settings, and a page whose issues
//! depend on the width the last `Emulation.setDeviceMetricsOverride` set - the width lease as
//! the app serves it - with Escape or a cancel at a chosen width. Every CDP call is recorded with
//! its view.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::{Value, json};

use crate::plan::Issue;
use crate::probe::{CLEAR, SET};

/// What the page shows at a width.
pub type Page = Box<dyn Fn(u32) -> Vec<Issue>>;

pub struct SweepHost {
    pub views: Vec<ViewInfo>,
    pub active: Option<ViewId>,
    pub settings: BTreeMap<String, String>,
    pub page: Page,
    /// The width the lease set last; `None` once ended.
    pub width: Option<u32>,
    /// Escape is pressed when the page is at this width.
    pub escape_at: Option<u32>,
    /// The lease is ended from outside (the cancel call) once this width was set.
    pub cancel_after: Option<u32>,
    /// The detector answers nothing a probe reads.
    pub broken: bool,
    /// How a hold (`show`) ends.
    pub hold_end: &'static str,
    /// Every script `evaluate` ran.
    pub scripts: Vec<String>,
    cancelled: bool,
    /// `(view, method, params)` of every CDP call.
    pub cdp: Vec<(ViewId, String, Value)>,
    /// The views `evaluate` ran in.
    pub evaluated: Vec<ViewId>,
    pub progress: Vec<(u32, u32)>,
    /// Every `findings.report`.
    pub reported: Vec<Vec<Finding>>,
}

impl Default for SweepHost {
    fn default() -> Self {
        Self {
            views: Vec::new(),
            active: None,
            settings: BTreeMap::new(),
            page: Box::new(|_| Vec::new()),
            width: None,
            escape_at: None,
            cancel_after: None,
            broken: false,
            hold_end: "elapsed",
            scripts: Vec::new(),
            cancelled: false,
            cdp: Vec::new(),
            evaluated: Vec::new(),
            progress: Vec::new(),
            reported: Vec::new(),
        }
    }
}

/// An overflow caused by `selector`.
pub fn overflow(selector: &str) -> Issue {
    Issue {
        kind: "overflow".to_owned(),
        selector: selector.to_owned(),
        path: format!("html > body > {selector}"),
        other: None,
    }
}

impl SweepHost {
    /// A host with the Chromium views `ids`, the first active, each showing `https://example.test/`.
    pub fn with_views(ids: &[ViewId]) -> Self {
        let mut host = Self::default();
        for &id in ids {
            host.views.push(ViewInfo {
                id,
                label: format!("View {id}"),
                breakpoint: format!("bp{id}"),
                engine: "chromium".to_owned(),
                width: 1280,
                height: 800,
                dpr: 1.0,
                url: "https://example.test/".to_owned(),
                title: String::new(),
            });
        }
        host.active = ids.first().copied();
        host
    }

    /// The setting `key` holds the JSON text `value`.
    pub fn set(&mut self, key: &str, value: &str) {
        self.settings.insert(key.to_owned(), value.to_owned());
    }

    /// The methods sent, in order.
    pub fn methods(&self) -> Vec<String> {
        self.cdp
            .iter()
            .map(|(_, method, _)| method.clone())
            .collect()
    }
}

impl PluginHost for SweepHost {
    fn log(&mut self, _level: &str, _message: &str) {}

    fn now_unix_seconds(&mut self) -> u64 {
        0
    }

    fn report_progress(&mut self, done: u32, total: u32) {
        self.progress.push((done, total));
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Ok(self.views.clone())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(self.active)
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.evaluated.push(view);
        self.scripts.push(script.to_owned());
        if self.broken {
            return Ok("null".to_owned());
        }
        let width = self.width.unwrap_or(1280);
        if script.contains(";WidthSweepDetect.hold(") {
            return Ok(json!({ "end": self.hold_end, "width": width }).to_string());
        }
        let issues = (self.page)(width);
        let answer = json!({
            "width": width,
            "escape": self.escape_at == Some(width),
            "issues": issues,
        });
        Ok(answer.to_string())
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        let parsed: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        self.cdp.push((view, method.to_owned(), parsed.clone()));
        match method {
            SET => {
                if self.cancelled {
                    return Err(Failure::coded(FailureKind::Cancelled, codes::CANCELLED));
                }
                let width = parsed["width"].as_u64().and_then(|w| u32::try_from(w).ok());
                self.width = width;
                if width.is_some() && width == self.cancel_after {
                    // The cancel call ends the lease while the sweep runs on.
                    self.cancelled = true;
                    self.width = None;
                }
                Ok("{}".to_owned())
            }
            CLEAR => {
                let ended = self.width.take().is_some();
                Ok(json!({ "ended": ended }).to_string())
            }
            _ => Err(Failure::coded(
                FailureKind::AccessDenied,
                codes::CDP_DOMAIN_DENIED,
            )),
        }
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, _path: &str) -> Result<Vec<u8>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn out_write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), Failure> {
        Err(Capability::OutWrite.denied())
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        self.reported.push(items.to_vec());
        Ok(())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
