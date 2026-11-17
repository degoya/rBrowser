//! A host for the native tests: views, the page script's answers per view (one for `collect`, one
//! for `apply`), settings, a clock, every evaluated script recorded, and the project's files the
//! plugin reads and writes (`.rbrowser/out/form-autofill/`), or no project at all.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::Value;

/// Where `out.write` puts the plugin's files.
pub const OUT: &str = ".rbrowser/out/form-autofill/";

fn no_project() -> Failure {
    Failure::coded(FailureKind::Other, codes::NOT_AVAILABLE)
}

#[derive(Debug, Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    /// What `collect` answers, by view; an unknown view answers not found.
    pub collected: BTreeMap<ViewId, Result<Value, Failure>>,
    /// What `apply` answers, by view; none answers `{}`.
    pub applied: BTreeMap<ViewId, Value>,
    /// Every script evaluated, as `(view, script)`.
    pub evaluated: Vec<(ViewId, String)>,
    pub settings: BTreeMap<String, String>,
    pub now: u64,
    pub progress: Vec<(u32, u32)>,
    /// The project's files by path (`.rbrowser/out/form-autofill/presets.json`).
    pub files: BTreeMap<String, Vec<u8>>,
    /// No project is open: reads and writes answer `plugins.error.not_available`.
    pub no_project: bool,
}

impl FakeHost {
    /// A view `id` labelled `label` of `engine`.
    pub fn view(&mut self, id: ViewId, label: &str, engine: &str) -> &mut Self {
        self.views.push(ViewInfo {
            id,
            label: label.to_owned(),
            breakpoint: label.to_ascii_lowercase(),
            engine: engine.to_owned(),
            width: 1280,
            height: 800,
            dpr: 1.0,
            url: "http://127.0.0.1:18767/forms/register.html".to_owned(),
            title: String::new(),
        });
        self
    }

    pub fn collect(&mut self, view: ViewId, answer: Value) -> &mut Self {
        self.collected.insert(view, Ok(answer));
        self
    }

    pub fn set(&mut self, key: &str, json: &str) -> &mut Self {
        self.settings.insert(key.to_owned(), json.to_owned());
        self
    }

    /// The plugin's own file `path` below `.rbrowser/out/form-autofill/`, as JSON.
    pub fn out_json(&self, path: &str) -> Option<Value> {
        let bytes = self.files.get(&format!("{OUT}{path}"))?;
        serde_json::from_slice(bytes).ok()
    }

    /// The request (JSON) of every `apply` sent to `view`.
    pub fn applies(&self, view: ViewId) -> Vec<Value> {
        self.requests(view)
            .into_iter()
            .filter(|request| request["op"] == "apply")
            .collect()
    }

    /// The request of every script sent to `view`.
    pub fn requests(&self, view: ViewId) -> Vec<Value> {
        self.evaluated
            .iter()
            .filter(|(id, _)| *id == view)
            .filter_map(|(_, script)| request_of(script))
            .collect()
    }
}

/// The request of a script: the JSON the page script is called with.
pub fn request_of(script: &str) -> Option<Value> {
    let start = script.rfind("\n)(")? + 3;
    serde_json::from_str(script.get(start..script.len().checked_sub(1)?)?).ok()
}

impl PluginHost for FakeHost {
    fn log(&mut self, _level: &str, _message: &str) {}

    fn now_unix_seconds(&mut self) -> u64 {
        self.now
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
        if request_of(script).is_some_and(|request| request["op"] == "apply") {
            return Ok(self
                .applied
                .get(&view)
                .cloned()
                .unwrap_or_else(|| Value::Object(serde_json::Map::new()))
                .to_string());
        }
        match self.collected.get(&view) {
            Some(Ok(value)) => Ok(value.to_string()),
            Some(Err(failure)) => Err(failure.clone()),
            None => Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)),
        }
    }

    fn cdp_send(&mut self, _view: ViewId, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Cdp.denied())
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        if self.no_project {
            return Err(no_project());
        }
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn project_list(&mut self, _dir: &str) -> Result<Vec<String>, Failure> {
        Err(Capability::ProjectRead.denied())
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        if self.no_project {
            return Err(no_project());
        }
        self.files.insert(format!("{OUT}{path}"), bytes.to_vec());
        Ok(())
    }

    fn report_findings(&mut self, _items: &[Finding]) -> Result<(), Failure> {
        Err(Capability::Findings.denied())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
