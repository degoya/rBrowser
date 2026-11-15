//! A host for the native tests: `MemoryHost` (grants, the `http` URL rule, answers by URL, every
//! call recorded) with an `evaluate` answer per script - the collector's and the offline
//! probe's - and views of the Chromium engine.

use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{
    Capability, Failure, Finding, HttpRequest, HttpResponse, PluginHost, ViewId, ViewInfo,
};
use serde_json::Value;

use crate::collect::COLLECTOR;

pub struct FakeHost {
    pub inner: MemoryHost,
    /// What the collector answers (JSON text).
    pub collector: String,
    /// What the offline probe answers (JSON text).
    pub offline: String,
}

impl Default for FakeHost {
    fn default() -> Self {
        let mut inner = MemoryHost::new()
            .grant(Capability::Views)
            .grant(Capability::Evaluate)
            .grant(Capability::Cdp)
            .grant(Capability::Http)
            .grant(Capability::OutWrite)
            .grant(Capability::Findings);
        inner.cdp_domains = vec!["Page".to_owned()];
        inner.cdp_answer = r#"{"installabilityErrors":[]}"#.to_owned();
        Self {
            inner,
            collector: "null".to_owned(),
            offline: "true".to_owned(),
        }
    }
}

impl FakeHost {
    /// A Chromium view `id` labelled `label` showing `url`.
    pub fn view(&mut self, id: ViewId, label: &str, url: &str) -> &mut Self {
        self.inner.views.push(ViewInfo {
            id,
            label: label.to_owned(),
            breakpoint: label.to_ascii_lowercase(),
            engine: "chromium".to_owned(),
            width: 1280,
            height: 800,
            dpr: 1.0,
            url: url.to_owned(),
            title: String::new(),
        });
        if self.inner.active.is_none() {
            self.inner.active = Some(id);
        }
        self
    }

    pub fn state(&mut self, state: &Value) -> &mut Self {
        self.collector = state.to_string();
        self
    }

    /// `url` answers `status` with `content_type` and `body`.
    pub fn serve(&mut self, url: &str, status: u16, content_type: &str, body: &[u8]) -> &mut Self {
        self.inner.responses.insert(
            url.to_owned(),
            HttpResponse {
                status,
                headers: vec![("content-type".to_owned(), content_type.to_owned())],
                body: body.to_vec(),
            },
        );
        self
    }

    /// The URLs requested, in order.
    pub fn fetched(&self) -> Vec<String> {
        self.inner
            .fetched
            .iter()
            .map(|request| request.url.clone())
            .collect()
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, level: &str, message: &str) {
        self.inner.log(level, message);
    }
    fn now_unix_seconds(&mut self) -> u64 {
        self.inner.now_unix_seconds()
    }
    fn report_progress(&mut self, done: u32, total: u32) {
        self.inner.report_progress(done, total);
    }
    fn setting(&mut self, key: &str) -> Option<String> {
        self.inner.setting(key)
    }
    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        self.inner.views()
    }
    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        self.inner.active_view()
    }
    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.inner.evaluate(view, script)?;
        Ok(if script == COLLECTOR {
            self.collector.clone()
        } else {
            self.offline.clone()
        })
    }
    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        self.inner.cdp_send(view, method, params)
    }
    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure> {
        self.inner.fetch(request)
    }
    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.inner.project_read(path)
    }
    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        self.inner.project_list(dir)
    }
    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.inner.out_write(path, bytes)
    }
    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        self.inner.report_findings(items)
    }
    fn sidecar_call(&mut self, method: &str, params: &str) -> Result<String, Failure> {
        self.inner.sidecar_call(method, params)
    }
}
