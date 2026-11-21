//! A host for the native tests: views with their served source (`Page.getFrameTree` and
//! `Page.getResourceContent` answered per view), their rendered DOM (the answer of
//! `source::render_script`), the runner's `evaluate` answers, settings, the project's rule file
//! and workspace trust (the host's gate on `project::TRUST_PROBE`), and a record of what the
//! plugin asked for.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::json;

/// The page every fake view shows.
pub const PAGE: &str = "http://localhost:5173/menu.html";

/// A fixture page from `tests/fixtures/<name>`.
pub fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).expect("the fixture exists")
}

/// The fake host.
#[derive(Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    /// Per view: the main frame's URL and the source `getResourceContent` answers, or its failure.
    pub sources: BTreeMap<ViewId, (String, Result<String, Failure>)>,
    /// Per view: the text `evaluate` answers, or its failure; `answer` for a view without one.
    pub answers: BTreeMap<ViewId, Result<String, Failure>>,
    pub answer: Option<String>,
    /// Per view: what the render script answers (`{"html": ...}`), or its failure.
    pub rendered: BTreeMap<ViewId, Result<String, Failure>>,
    /// What `project.read` of the rule file answers; not found when `None`.
    pub project: Option<Result<Vec<u8>, Failure>>,
    /// Whether the workspace is trusted.
    pub trusted: bool,
    pub project_reads: Vec<String>,
    /// Settings by key, JSON text.
    pub settings: BTreeMap<String, String>,
    /// Whether `findings.report` is wired; `plugins.error.not_available` when not.
    pub findings_wired: bool,
    pub evaluated: Vec<(ViewId, String)>,
    pub cdp: Vec<(ViewId, String, String)>,
    pub reported: Vec<Finding>,
    pub progress: Vec<(u32, u32)>,
    pub logs: Vec<(String, String)>,
}

impl FakeHost {
    /// A host with `views` as (id, breakpoint), each serving `source`, the runner answering
    /// `answer` for every view.
    pub fn with_views(views: &[(ViewId, &str)], source: &str, answer: &str) -> Self {
        let mut host = Self {
            findings_wired: true,
            answer: Some(answer.to_owned()),
            ..Self::default()
        };
        for (id, breakpoint) in views {
            host.views.push(ViewInfo {
                id: *id,
                label: format!("View {id}"),
                breakpoint: (*breakpoint).to_owned(),
                engine: "chromium".to_owned(),
                width: 375,
                height: 800,
                dpr: 1.0,
                url: PAGE.to_owned(),
                title: "Menu".to_owned(),
            });
            host.sources
                .insert(*id, (PAGE.to_owned(), Ok(source.to_owned())));
        }
        host
    }

    /// Sets the plugin's setting `key` to the JSON `value`.
    pub fn set(mut self, key: &str, value: &str) -> Self {
        self.settings.insert(key.to_owned(), value.to_owned());
        self
    }
}

fn param(params: &str, name: &str) -> String {
    serde_json::from_str::<serde_json::Value>(params)
        .ok()
        .and_then(|value| value[name].as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl PluginHost for FakeHost {
    fn log(&mut self, level: &str, message: &str) {
        self.logs.push((level.to_owned(), message.to_owned()));
    }

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
        Ok(self.views.first().map(|view| view.id))
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.evaluated.push((view, script.to_owned()));
        if script == crate::source::render_script() {
            return self
                .rendered
                .get(&view)
                .cloned()
                .unwrap_or_else(|| Err(Failure::new(FailureKind::NotFound)));
        }
        match (self.answers.get(&view), &self.answer) {
            (Some(answer), _) => answer.clone(),
            (None, Some(answer)) => Ok(answer.clone()),
            (None, None) => Err(Failure::new(FailureKind::NotFound)),
        }
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        self.cdp.push((view, method.to_owned(), params.to_owned()));
        let Some((url, source)) = self.sources.get(&view) else {
            return Err(Failure::engine_unserved("cdp", view, "webkit"));
        };
        match method {
            "Page.getFrameTree" => Ok(json!({
                "frameTree": { "frame": { "id": format!("F{view}"), "url": url } }
            })
            .to_string()),
            "Page.getResourceContent" => {
                assert_eq!(param(params, "frameId"), format!("F{view}"));
                assert_eq!(param(params, "url"), *url);
                source
                    .clone()
                    .map(|text| json!({ "content": text, "base64Encoded": false }).to_string())
            }
            _ => Err(Failure::new(FailureKind::AccessDenied)),
        }
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.project_reads.push(path.to_owned());
        match &self.project {
            Some(answer) if path == crate::project::PROJECT_FILE => answer.clone(),
            _ => Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)),
        }
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        if dir == crate::project::TRUST_PROBE && !self.trusted {
            return Err(Failure::coded(
                FailureKind::AccessDenied,
                codes::UNTRUSTED_WORKSPACE,
            ));
        }
        Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn out_write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), Failure> {
        Err(Capability::OutWrite.denied())
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        if !self.findings_wired {
            return Err(Failure::coded(
                FailureKind::NotSupported,
                rb_plugin_api::codes::NOT_AVAILABLE,
            ));
        }
        self.reported.extend_from_slice(items);
        Ok(())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
