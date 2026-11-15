//! A host for the unit tests: CDP answers per view and method, a clock that can run, settings,
//! the project's files and what `out.write` wrote, and a workspace that may be untrusted -
//! `project.list` and `project.read` of `.rbrowser/snippets/` then answer
//! `plugins.error.untrusted_workspace`, as the real host does.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};

#[derive(Debug, Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    pub active: Option<ViewId>,
    /// What `cdp.send` answers, by `(view, method)`; others answer `{}`.
    pub cdp: BTreeMap<(ViewId, String), Result<String, Failure>>,
    pub settings: BTreeMap<String, String>,
    /// Project files by project-relative path; the out folder lies below `.rbrowser/out/snippets/`.
    pub files: BTreeMap<String, Vec<u8>>,
    pub trusted: bool,
    /// No project is open: every file call answers `plugins.error.not_available`.
    pub no_project: bool,
    pub now: u64,
    /// Seconds the clock moves at each reading.
    pub tick: u64,
    pub sent: Vec<(ViewId, String, String)>,
    pub logs: Vec<(String, String)>,
}

/// A Chromium view.
pub fn view(id: ViewId, label: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: label.to_owned(),
        breakpoint: label.to_lowercase(),
        engine: "chromium".to_owned(),
        width: 375 * id,
        height: 800,
        dpr: 1.0,
        url: "http://127.0.0.1:8080/".to_owned(),
        title: "Fixture".to_owned(),
    }
}

/// The answer of `Runtime.evaluate` whose wrapper gave `value`.
pub fn evaluated(value: &serde_json::Value) -> String {
    serde_json::json!({ "result": { "type": "object", "value": value } }).to_string()
}

impl FakeHost {
    pub fn with_views(views: Vec<ViewInfo>) -> Self {
        Self {
            views,
            trusted: true,
            now: 1_731_300_000,
            ..Self::default()
        }
    }

    pub fn set(&mut self, key: &str, json: &str) {
        self.settings.insert(key.to_owned(), json.to_owned());
    }

    pub fn file(&mut self, path: &str, text: &str) {
        self.files.insert(path.to_owned(), text.as_bytes().to_vec());
    }

    pub fn answer(&mut self, view: ViewId, method: &str, answer: Result<String, Failure>) {
        self.cdp.insert((view, method.to_owned()), answer);
    }

    /// The methods sent, in order.
    pub fn methods(&self) -> Vec<String> {
        self.sent
            .iter()
            .map(|(_, method, _)| method.clone())
            .collect()
    }

    fn gate(&self, path: &str) -> Result<(), Failure> {
        if self.no_project {
            return Err(Failure::coded(
                FailureKind::NotSupported,
                codes::NOT_AVAILABLE,
            ));
        }
        if !self.trusted && path.starts_with(".rbrowser/snippets") {
            return Err(Failure::coded(
                FailureKind::AccessDenied,
                codes::UNTRUSTED_WORKSPACE,
            ));
        }
        Ok(())
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, level: &str, message: &str) {
        self.logs.push((level.to_owned(), message.to_owned()));
    }

    fn now_unix_seconds(&mut self) -> u64 {
        let now = self.now;
        self.now += self.tick;
        now
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

    fn evaluate(&mut self, _view: ViewId, _script: &str) -> Result<String, Failure> {
        Err(Capability::Evaluate.denied())
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        self.sent.push((view, method.to_owned(), params.to_owned()));
        self.cdp
            .get(&(view, method.to_owned()))
            .cloned()
            .unwrap_or_else(|| Ok("{}".to_owned()))
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.gate(path)?;
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        self.gate(dir)?;
        let prefix = format!("{dir}/");
        let names: Vec<String> = self
            .files
            .keys()
            .filter_map(|path| path.strip_prefix(&prefix))
            .map(|rest| match rest.split_once('/') {
                Some((folder, _)) => format!("{folder}/"),
                None => rest.to_owned(),
            })
            .collect();
        if names.is_empty() {
            return Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND));
        }
        let mut names = names;
        names.dedup();
        Ok(names)
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.gate(path)?;
        self.files
            .insert(format!(".rbrowser/out/snippets/{path}"), bytes.to_vec());
        Ok(())
    }

    fn report_findings(&mut self, _items: &[Finding]) -> Result<(), Failure> {
        Err(Capability::Findings.denied())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
