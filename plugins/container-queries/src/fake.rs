//! A host for the native tests: `MemoryHost` of `rb-plugin-api` answers one text for every view;
//! the analysis needs one per view. Everything else is the memory host's.

use std::collections::BTreeMap;

use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Failure, Finding, HttpRequest, HttpResponse, PluginHost, ViewId, ViewInfo};

#[derive(Debug, Default)]
pub struct PerView {
    pub host: MemoryHost,
    /// What `evaluate` answers, by view; another view gets the memory host's answer.
    pub answers: BTreeMap<ViewId, String>,
}

impl PluginHost for PerView {
    fn log(&mut self, level: &str, message: &str) {
        self.host.log(level, message);
    }

    fn now_unix_seconds(&mut self) -> u64 {
        self.host.now_unix_seconds()
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.host.setting(key)
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        self.host.views()
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        self.host.active_view()
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        let fallback = self.host.evaluate(view, script)?;
        Ok(self.answers.get(&view).cloned().unwrap_or(fallback))
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        self.host.cdp_send(view, method, params)
    }

    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, Failure> {
        self.host.fetch(request)
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.host.project_read(path)
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        self.host.project_list(dir)
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.host.out_write(path, bytes)
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        self.host.report_findings(items)
    }

    fn sidecar_call(&mut self, method: &str, params: &str) -> Result<String, Failure> {
        self.host.sidecar_call(method, params)
    }
}
