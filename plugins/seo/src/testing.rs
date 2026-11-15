//! Helpers of the unit tests: a page served by a `MemoryHost`, checked, its findings in short.

use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, Finding, HttpResponse};
use serde_json::json;

use crate::audit::check_page;
use crate::config::Config;
use crate::rendered::Rendered;
use crate::rules::Sink;
use crate::site::Site;
use crate::url::Url;

/// A host granting `http` and `evaluate`, serving `html` at `url` with `headers`.
#[must_use]
pub fn serve(url: &str, html: &str, headers: &[(&str, &str)]) -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Http)
        .grant(Capability::Evaluate);
    let mut all = vec![(
        "content-type".to_owned(),
        "text/html; charset=utf-8".to_owned(),
    )];
    all.extend(
        headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned())),
    );
    host.responses.insert(
        url.to_owned(),
        HttpResponse {
            status: 200,
            headers: all,
            body: html.as_bytes().to_vec(),
        },
    );
    host
}

/// A rendered page of view 1 showing `url` with `html`.
#[must_use]
pub fn rendered(url: &str, html: &str) -> Rendered {
    let answer = json!({"url": url, "html": html, "images": [], "backgrounds": []});
    Rendered::parse(1, &answer.to_string()).expect("a collection")
}

/// The findings of checking `url` on `host` (as a crawl's later page: no share image fetched).
#[must_use]
pub fn check(host: &mut MemoryHost, url: &str, rendered: Option<&Rendered>) -> Vec<Finding> {
    check_with(host, &Config::default(), url, rendered, false)
}

/// The findings of checking `url` on `host` with `config`; `start` fetches the share image as a
/// run's start page does.
#[must_use]
pub fn check_with(
    host: &mut MemoryHost,
    config: &Config,
    url: &str,
    rendered: Option<&Rendered>,
    start: bool,
) -> Vec<Finding> {
    let page = Url::parse(url).expect("url");
    let site = Site::load(host, &page);
    let enabled = Vec::new();
    let mut sink = Sink::new(config, &enabled);
    let _ = check_page(host, config, &site, &page, rendered, start, &mut sink);
    sink.findings
}

/// `rule message-suffix` of every finding whose rule starts with `group`.
#[must_use]
pub fn short(findings: &[Finding], group: &str) -> Vec<String> {
    findings
        .iter()
        .filter(|finding| finding.rule.starts_with(group))
        .map(|finding| {
            format!(
                "{} {}",
                finding.rule,
                finding
                    .message
                    .strip_prefix("plugin.seo.findings.")
                    .unwrap_or(&finding.message)
            )
        })
        .collect()
}

/// A `MemoryHost` whose `evaluate` answers per view (the parity of the views).
pub struct ViewsHost {
    pub inner: MemoryHost,
    /// The collector's answer by view.
    pub answers: std::collections::BTreeMap<rb_plugin_api::ViewId, String>,
}

impl rb_plugin_api::PluginHost for ViewsHost {
    fn log(&mut self, level: &str, message: &str) {
        self.inner.log(level, message);
    }
    fn now_unix_seconds(&mut self) -> u64 {
        self.inner.now_unix_seconds()
    }
    fn setting(&mut self, key: &str) -> Option<String> {
        self.inner.setting(key)
    }
    fn views(&mut self) -> Result<Vec<rb_plugin_api::ViewInfo>, rb_plugin_api::Failure> {
        self.inner.views()
    }
    fn active_view(&mut self) -> Result<Option<rb_plugin_api::ViewId>, rb_plugin_api::Failure> {
        self.inner.active_view()
    }
    fn evaluate(
        &mut self,
        view: rb_plugin_api::ViewId,
        script: &str,
    ) -> Result<String, rb_plugin_api::Failure> {
        self.inner.evaluate(view, script)?;
        Ok(self
            .answers
            .get(&view)
            .cloned()
            .unwrap_or_else(|| "null".to_owned()))
    }
    fn cdp_send(
        &mut self,
        view: rb_plugin_api::ViewId,
        method: &str,
        params: &str,
    ) -> Result<String, rb_plugin_api::Failure> {
        self.inner.cdp_send(view, method, params)
    }
    fn fetch(
        &mut self,
        request: &rb_plugin_api::HttpRequest,
    ) -> Result<HttpResponse, rb_plugin_api::Failure> {
        self.inner.fetch(request)
    }
    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, rb_plugin_api::Failure> {
        self.inner.project_read(path)
    }
    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, rb_plugin_api::Failure> {
        self.inner.project_list(dir)
    }
    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), rb_plugin_api::Failure> {
        self.inner.out_write(path, bytes)
    }
    fn report_findings(&mut self, items: &[Finding]) -> Result<(), rb_plugin_api::Failure> {
        self.inner.report_findings(items)
    }
    fn sidecar_call(
        &mut self,
        method: &str,
        params: &str,
    ) -> Result<String, rb_plugin_api::Failure> {
        self.inner.sidecar_call(method, params)
    }
}
