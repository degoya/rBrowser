//! The tests' host: `MemoryHost` with the plugin's grants, Chromium views and the collector's
//! answer for a page; [`PerView`] answers the collector per view.

use std::collections::BTreeMap;

use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{
    Capability, Failure, Finding, HttpRequest, HttpResponse, PluginHost, ViewId, ViewInfo,
};
use serde_json::json;

pub const URL: &str = "https://example.test/";

/// A page with an `Organization`, an `Article` with a misspelled property and a Microdata item.
pub const PAGE: &str = r##"<html><head>
<script type="application/ld+json">{"@context": "https://schema.org", "@type": "Organization", "@id": "#org", "name": "Example", "url": "https://example.test/", "logo": "https://example.test/logo.png"}</script>
<script type="application/ld+json">{"@context": "https://schema.org", "@type": "Article", "headline": "Hello", "publisher": {"@id": "#org"}, "author": {"@type": "Person", "name": "Ada"}, "datePublished": "2026-01-02", "dateModified": "2026-01-03", "image": "https://example.test/a.png", "headlinee": "typo"}</script>
</head><body><div itemscope itemtype="https://schema.org/Thing"><span itemprop="name">Lamp</span></div></body></html>"##;

/// A host granted what the manifest grants, whose views show `html`.
pub fn host(html: &str) -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::Findings);
    host.evaluate_answer = answer(URL, html);
    host
}

/// Adds a Chromium view `id` showing `url`; the first one is active.
pub fn view(host: &mut MemoryHost, id: ViewId, url: &str) {
    host.views.push(ViewInfo {
        id,
        label: format!("View {id}"),
        breakpoint: format!("bp{id}"),
        engine: "chromium".to_owned(),
        width: 1280,
        height: 800,
        dpr: 1.0,
        url: url.to_owned(),
        title: String::new(),
    });
    if host.active.is_none() {
        host.active = Some(id);
    }
}

/// The collector's answer for a view showing `html` at `url`.
pub fn answer(url: &str, html: &str) -> String {
    json!({ "url": url, "length": html.len(), "html": html }).to_string()
}

/// `MemoryHost` whose `evaluate` answers per view: the answer or failure `answers` holds for the
/// view, the inner host's answer for any other.
pub struct PerView {
    pub inner: MemoryHost,
    pub answers: BTreeMap<ViewId, Result<String, Failure>>,
}

impl PluginHost for PerView {
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
        let inner = self.inner.evaluate(view, script)?;
        self.answers.get(&view).cloned().unwrap_or(Ok(inner))
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
