//! A host for the unit tests: views, settings, one file store behind `project-read` and
//! `out-write` (the out folder lies below `.rbrowser/`, as in the real host), and canned answers
//! for `cdp`.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};

/// The plugin's out folder, relative to the project.
pub const OUT: &str = ".rbrowser/out/network-panel/";

#[derive(Debug, Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    pub settings: BTreeMap<String, String>,
    /// Project files by project-relative path.
    pub files: BTreeMap<String, Vec<u8>>,
    /// `cdp` answers by `(view, request id)` of `Network.getResponseBody`.
    pub bodies: BTreeMap<(ViewId, String), String>,
    pub now: u64,
    pub logs: Vec<(String, String)>,
    pub cdp_calls: Vec<(ViewId, String, String)>,
    /// What `findings.report` received (RB-096-06).
    pub findings: Vec<Finding>,
}

/// A Chromium view of `width` pixels.
pub fn view(id: ViewId, label: &str, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: label.to_owned(),
        breakpoint: label.to_lowercase(),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: "http://127.0.0.1:8080/network/index.html".to_owned(),
        title: "Network fixture".to_owned(),
    }
}

fn not_found() -> Failure {
    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)
}

impl FakeHost {
    pub fn with_views(views: Vec<ViewInfo>) -> Self {
        Self {
            views,
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

    /// A file of the plugin's out folder, as text.
    pub fn out(&self, path: &str) -> Option<String> {
        self.files
            .get(&format!("{OUT}{path}"))
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
    }
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
        Ok(self.views.clone())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(self.views.first().map(|view| view.id))
    }

    fn evaluate(&mut self, _view: ViewId, _script: &str) -> Result<String, Failure> {
        Err(Capability::Evaluate.denied())
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        self.cdp_calls
            .push((view, method.to_owned(), params.to_owned()));
        let params: serde_json::Value = serde_json::from_str(params).unwrap_or_default();
        let id = params["requestId"].as_str().unwrap_or_default().to_owned();
        match method {
            "Network.getResponseBody" => self.bodies.get(&(view, id)).cloned().ok_or_else(|| {
                Failure::coded(FailureKind::Other, codes::HOST_FAILED)
                    .with_param("reason", "No resource with given identifier found")
            }),
            _ => Err(not_found()),
        }
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.files.get(path).cloned().ok_or_else(not_found)
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        let prefix = format!("{}/", dir.trim_end_matches('/'));
        let mut names: Vec<String> = self
            .files
            .keys()
            .filter_map(|path| path.strip_prefix(&prefix))
            .map(|rest| match rest.split_once('/') {
                Some((folder, _)) => format!("{folder}/"),
                None => rest.to_owned(),
            })
            .collect();
        names.dedup();
        if names.is_empty() {
            return Err(not_found());
        }
        Ok(names)
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.files.insert(format!("{OUT}{path}"), bytes.to_vec());
        Ok(())
    }

    fn report_findings(&mut self, items: &[Finding]) -> Result<(), Failure> {
        self.findings.extend_from_slice(items);
        Ok(())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
