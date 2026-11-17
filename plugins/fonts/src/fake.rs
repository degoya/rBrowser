//! A host for the native tests: views, the collector's answer per view, CDP answers (the document,
//! a node per selector, the platform fonts per node), the files written, every call recorded.

use std::collections::BTreeMap;

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::{Value, json};

#[derive(Debug, Default)]
pub struct FakeHost {
    pub views: Vec<ViewInfo>,
    /// What the collector answers, by view (JSON text); an unknown view answers not found.
    pub pages: BTreeMap<ViewId, String>,
    /// What the marker answers, by view; `{"count": 0}` when not set.
    pub marks: BTreeMap<ViewId, Value>,
    /// Every script evaluated, as `(view, script)`.
    pub evaluated: Vec<(ViewId, String)>,
    /// CDP answers by method, before the defaults below.
    pub cdp: BTreeMap<String, Result<Value, Failure>>,
    /// How often a method fails (node ids dropped) before it answers.
    pub fail_first: BTreeMap<String, usize>,
    /// `DOM.querySelector`: the node of a selector (0 when unknown).
    pub nodes: BTreeMap<String, u64>,
    /// `CSS.getPlatformFontsForNode`: the fonts of a node (none when unknown).
    pub fonts: BTreeMap<u64, Value>,
    pub cdp_sent: Vec<(ViewId, String, Value)>,
    pub settings: BTreeMap<String, String>,
    pub written: Vec<(String, Vec<u8>)>,
    pub findings: Vec<Finding>,
    pub now: u64,
}

impl FakeHost {
    /// A Chromium view `id` labelled `label`, `width` px wide, showing `url`.
    pub fn view(&mut self, id: ViewId, label: &str, width: u32, url: &str) -> &mut Self {
        self.views.push(ViewInfo {
            id,
            label: label.to_owned(),
            breakpoint: label.to_ascii_lowercase(),
            engine: "chromium".to_owned(),
            width,
            height: 800,
            dpr: 1.0,
            url: url.to_owned(),
            title: String::new(),
        });
        self
    }

    pub fn set(&mut self, key: &str, json_text: &str) -> &mut Self {
        self.settings.insert(key.to_owned(), json_text.to_owned());
        self
    }

    /// `selector` is node `node`, rendered with `fonts` (`[[family, custom, glyphs], ...]`).
    pub fn rendered(
        &mut self,
        selector: &str,
        node: u64,
        fonts: &[(&str, bool, u64)],
    ) -> &mut Self {
        self.nodes.insert(selector.to_owned(), node);
        let list: Vec<Value> = fonts
            .iter()
            .map(|(family, custom, glyphs)| {
                json!({ "familyName": family, "postScriptName": family, "isCustomFont": custom, "glyphCount": glyphs })
            })
            .collect();
        self.fonts.insert(node, Value::Array(list));
        self
    }

    pub fn methods(&self) -> Vec<&str> {
        self.cdp_sent.iter().map(|(_, m, _)| m.as_str()).collect()
    }

    fn cdp_default(&self, method: &str, params: &Value) -> Value {
        match method {
            "Accessibility.getFullAXTree" => json!({ "nodes": [{ "backendDOMNodeId": 7 }] }),
            "DOM.pushNodesByBackendIdsToFrontend" => json!({ "nodeIds": [1] }),
            "DOM.getDocument" => json!({ "root": { "nodeId": 1 } }),
            "DOM.querySelector" => {
                let selector = params["selector"].as_str().unwrap_or_default();
                json!({ "nodeId": self.nodes.get(selector).copied().unwrap_or(0) })
            }
            "CSS.getPlatformFontsForNode" => {
                let node = params["nodeId"].as_u64().unwrap_or_default();
                json!({ "fonts": self.fonts.get(&node).cloned().unwrap_or_else(|| json!([])) })
            }
            _ => json!({}),
        }
    }
}

impl PluginHost for FakeHost {
    fn log(&mut self, _level: &str, _message: &str) {}

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

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.evaluated.push((view, script.to_owned()));
        if let Some(info) = self.views.iter().find(|info| info.id == view)
            && info.engine != "chromium"
        {
            return Err(Failure::coded(
                FailureKind::NotSupported,
                codes::NOT_AVAILABLE,
            ));
        }
        if script.contains("FontsPage.mark(") {
            return Ok(self
                .marks
                .get(&view)
                .cloned()
                .unwrap_or_else(|| json!({ "count": 0 }))
                .to_string());
        }
        if script.contains("FontsPage.clear(") {
            return Ok(json!({ "cleared": true }).to_string());
        }
        self.pages
            .get(&view)
            .cloned()
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        let params: Value = serde_json::from_str(params).unwrap_or(Value::Null);
        self.cdp_sent
            .push((view, method.to_owned(), params.clone()));
        if let Some(info) = self.views.iter().find(|info| info.id == view)
            && info.engine != "chromium"
        {
            return Err(Failure::coded(
                FailureKind::NotSupported,
                codes::NOT_AVAILABLE,
            ));
        }
        if let Some(left) = self.fail_first.get_mut(method)
            && *left > 0
        {
            *left -= 1;
            return Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED));
        }
        match self.cdp.get(method) {
            Some(Ok(value)) => Ok(value.to_string()),
            Some(Err(failure)) => Err(failure.clone()),
            None => Ok(self.cdp_default(method, &params).to_string()),
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

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.written.push((path.to_owned(), bytes.to_vec()));
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

/// The collector's committed answer for the fixture page (`wide` or `narrow`).
pub fn fixture(name: &str) -> crate::page::PageFonts {
    let text = match name {
        "wide" => include_str!("../tests/fixtures/wide.json"),
        _ => include_str!("../tests/fixtures/narrow.json"),
    };
    crate::page::PageFonts::parse(text).expect("the committed fixture parses")
}

/// One view's fonts as a run collects them.
pub fn view_fonts(
    id: ViewId,
    label: &str,
    width: u32,
    fonts: crate::page::PageFonts,
    rendered: crate::run::Rendered,
) -> crate::run::ViewFonts {
    let mut host = FakeHost::default();
    host.view(id, label, width, &fonts.url);
    crate::run::ViewFonts {
        view: host.views.remove(0),
        fonts,
        rendered,
    }
}
