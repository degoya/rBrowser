//! A scripted browser behind the plugin's host: views in sessions, a cookie jar per session
//! (CDP `Network.getCookies`, `setCookie`, `deleteCookies` as Chromium answers them), a
//! localStorage per session and origin, a sessionStorage per view and origin, and the operations
//! of `src/ops.js` carried out on them. Firefox and WebKit views answer `not_available`, as the
//! real host does. Shared by the unit tests (`src/lib.rs`) and the fixture test (`tests/`).
#![allow(dead_code, reason = "each test binary uses a part of the fake")]

use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, ViewId,
    ViewInfo, codes,
};
use serde_json::{Value, json};
use storage_panel::origin;
use storage_panel::read::OPS;

/// The sessionStorage key `src/ops.js` keeps the pins of a tab in.
pub const RECORD: &str = "rbrowser:storage-panel:pins";

#[derive(Debug, Default)]
pub struct Browser {
    pub views: Vec<ViewInfo>,
    /// The session each view really is in (the app's truth, whatever the config says).
    pub view_sessions: BTreeMap<ViewId, String>,
    /// Cookies in CDP's shape, per session.
    pub jars: BTreeMap<String, Vec<Value>>,
    /// localStorage per (session, origin).
    pub local: BTreeMap<(String, String), BTreeMap<String, String>>,
    /// sessionStorage per (view, origin).
    pub session: BTreeMap<(ViewId, String), BTreeMap<String, String>>,
    pub settings: BTreeMap<String, String>,
    pub project: BTreeMap<String, Vec<u8>>,
    pub logs: Vec<(String, String)>,
    pub cdp_sent: Vec<(ViewId, String, Value)>,
    pub evaluated: Vec<(ViewId, Value)>,
    /// Views whose CDP session fails.
    pub broken: BTreeSet<ViewId>,
    /// The workspace is not trusted: the protected folders refuse, as the host's do.
    pub untrusted: bool,
}

pub fn view(id: ViewId, breakpoint: &str, engine: &str, url: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: format!("View {id}"),
        breakpoint: breakpoint.to_owned(),
        engine: engine.to_owned(),
        width: 400 * id,
        height: 800,
        dpr: 1.0,
        url: url.to_owned(),
        title: "Storage fixture".to_owned(),
    }
}

fn not_available() -> Failure {
    Failure::coded(FailureKind::NotSupported, codes::NOT_AVAILABLE).with_param("engine", "other")
}

impl Browser {
    pub fn add_view(&mut self, info: ViewInfo, session: &str) {
        self.view_sessions.insert(info.id, session.to_owned());
        self.views.push(info);
    }

    pub fn session_of(&self, view: ViewId) -> String {
        self.view_sessions.get(&view).cloned().unwrap_or_default()
    }

    fn url(&self, view: ViewId) -> String {
        self.views
            .iter()
            .find(|info| info.id == view)
            .map(|info| info.url.clone())
            .unwrap_or_default()
    }

    pub fn navigate(&mut self, view: ViewId, url: &str) {
        if let Some(info) = self.views.iter_mut().find(|info| info.id == view) {
            info.url = url.to_owned();
        }
    }

    /// The origin of the view's document, `"null"` for a page without one.
    pub fn origin(&self, view: ViewId) -> String {
        origin::origin_of(&self.url(view)).unwrap_or_else(|| "null".to_owned())
    }

    /// A cookie the server (or the page) sets in the view's session.
    pub fn set_cookie(&mut self, view: ViewId, cookie: Value) {
        let session = self.session_of(view);
        let jar = self.jars.entry(session).or_default();
        jar.retain(|old| {
            (&old["name"], &old["domain"], &old["path"])
                != (&cookie["name"], &cookie["domain"], &cookie["path"])
        });
        jar.push(cookie);
    }

    pub fn local_item(&self, view: ViewId, key: &str) -> Option<String> {
        self.local
            .get(&(self.session_of(view), self.origin(view)))
            .and_then(|items| items.get(key).cloned())
    }

    pub fn set_local(&mut self, view: ViewId, key: &str, value: &str) {
        let at = (self.session_of(view), self.origin(view));
        self.local
            .entry(at)
            .or_default()
            .insert(key.to_owned(), value.to_owned());
    }

    pub fn session_item(&self, view: ViewId, key: &str) -> Option<String> {
        self.session
            .get(&(view, self.origin(view)))
            .and_then(|items| items.get(key).cloned())
    }

    /// What page/pins.js does at document start: the tab's pins record set again.
    pub fn run_page_script(&mut self, view: ViewId) {
        let Some(text) = self.session_item(view, RECORD) else {
            return;
        };
        let record: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        self.apply(view, &record["local"], &record["session"]);
    }

    /// What page/pins.js does with the plugin's answer to `loaded`: the record kept (or dropped)
    /// and its values set.
    pub fn take_page_answer(&mut self, view: ViewId, record: &Value) {
        let origin = self.origin(view);
        let tab = self.session.entry((view, origin)).or_default();
        if record.is_null() {
            tab.remove(RECORD);
            return;
        }
        tab.insert(RECORD.to_owned(), record.to_string());
        self.apply(view, &record["local"], &record["session"]);
    }

    fn apply(&mut self, view: ViewId, local: &Value, session: &Value) {
        let origin = self.origin(view);
        for (key, value) in local.as_object().into_iter().flatten() {
            let at = (self.session_of(view), origin.clone());
            self.local
                .entry(at)
                .or_default()
                .insert(key.clone(), value.as_str().unwrap_or_default().to_owned());
        }
        for (key, value) in session.as_object().into_iter().flatten() {
            self.session
                .entry((view, origin.clone()))
                .or_default()
                .insert(key.clone(), value.as_str().unwrap_or_default().to_owned());
        }
    }

    pub fn cookies_of(&self, view: ViewId) -> Vec<Value> {
        let url = self.url(view);
        let host = origin::host_of(&url).unwrap_or_default();
        let path = origin::path_of(&url);
        self.jars
            .get(&self.session_of(view))
            .into_iter()
            .flatten()
            .filter(|cookie| {
                origin::domain_matches(&host, cookie["domain"].as_str().unwrap_or_default())
                    && origin::path_matches(&path, cookie["path"].as_str().unwrap_or("/"))
            })
            .cloned()
            .collect()
    }

    fn supported(&self, view: ViewId) -> Result<(), Failure> {
        match self.views.iter().find(|info| info.id == view) {
            None => Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND)),
            Some(info) if info.engine != "chromium" => Err(not_available()),
            Some(_) if self.broken.contains(&view) => {
                Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED)
                    .with_param("reason", "detached"))
            }
            Some(_) => Ok(()),
        }
    }

    fn run_op(&mut self, view: ViewId, op: &Value) -> Value {
        let origin = self.origin(view);
        let session = self.session_of(view);
        if op["op"] == "read" {
            if origin == "null" {
                return json!({ "ok": false, "reason": "unavailable", "origin": "null" });
            }
            let list = |items: Option<&BTreeMap<String, String>>| -> Vec<Value> {
                items
                    .into_iter()
                    .flatten()
                    .filter(|(key, _)| key.as_str() != RECORD)
                    .map(|(key, value)| json!([key, value]))
                    .collect()
            };
            let local = if op["local"] == true {
                list(self.local.get(&(session, origin.clone())))
            } else {
                Vec::new()
            };
            let tab = if op["session"] == true {
                list(self.session.get(&(view, origin.clone())))
            } else {
                Vec::new()
            };
            return json!({ "origin": origin, "local": local, "session": tab, "more": 0 });
        }
        if op["origin"] != json!(origin) {
            return json!({ "ok": false, "reason": "origin", "origin": origin });
        }
        let key = op["key"].as_str().unwrap_or_default().to_owned();
        let tab = op["area"] == "session";
        let items = if tab {
            self.session.entry((view, origin)).or_default()
        } else {
            self.local.entry((session, origin)).or_default()
        };
        match op["op"].as_str() {
            Some("set") => {
                items.insert(key, op["value"].as_str().unwrap_or_default().to_owned());
            }
            Some("remove") => {
                items.remove(&key);
            }
            Some("pins") => {
                self.apply(view, &op["local"], &op["session"]);
                let origin = self.origin(view);
                let empty = op["local"].as_object().is_none_or(|m| m.is_empty())
                    && op["session"].as_object().is_none_or(|m| m.is_empty());
                let tab = self.session.entry((view, origin.clone())).or_default();
                if empty {
                    tab.remove(RECORD);
                } else {
                    tab.insert(
                        RECORD.to_owned(),
                        json!({ "origin": origin, "local": op["local"], "session": op["session"] })
                            .to_string(),
                    );
                }
            }
            _ => return json!({ "ok": false, "reason": "op" }),
        }
        json!({ "ok": true })
    }
}

impl PluginHost for Browser {
    fn log(&mut self, level: &str, message: &str) {
        self.logs.push((level.to_owned(), message.to_owned()));
    }

    fn now_unix_seconds(&mut self) -> u64 {
        1_731_300_000
    }

    fn setting(&mut self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    fn views(&mut self) -> Result<Vec<ViewInfo>, Failure> {
        Ok(self.views.clone())
    }

    fn active_view(&mut self) -> Result<Option<ViewId>, Failure> {
        Ok(self.views.first().map(|info| info.id))
    }

    fn evaluate(&mut self, view: ViewId, script: &str) -> Result<String, Failure> {
        self.supported(view)?;
        let prefix = format!("({})(", OPS.trim());
        let op: Value = script
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(')'))
            .and_then(|json| serde_json::from_str(json).ok())
            .expect("the plugin runs only src/ops.js");
        self.evaluated.push((view, op.clone()));
        Ok(self.run_op(view, &op).to_string())
    }

    fn cdp_send(&mut self, view: ViewId, method: &str, params: &str) -> Result<String, Failure> {
        self.supported(view)?;
        let params: Value = serde_json::from_str(params).expect("CDP params are JSON");
        self.cdp_sent
            .push((view, method.to_owned(), params.clone()));
        match method {
            "Network.getCookies" => Ok(json!({ "cookies": self.cookies_of(view) }).to_string()),
            "Network.setCookie" => {
                let (domain, path) = match params["domain"].as_str() {
                    Some(domain) => (domain.to_owned(), params["path"].clone()),
                    None => {
                        let url = params["url"].as_str().unwrap_or_default();
                        (
                            origin::host_of(url).unwrap_or_default(),
                            params["path"].clone(),
                        )
                    }
                };
                let mut cookie = params.clone();
                cookie["domain"] = json!(domain);
                cookie["path"] = if path.is_null() { json!("/") } else { path };
                cookie["session"] = json!(params.get("expires").is_none());
                if let Some(object) = cookie.as_object_mut() {
                    object.remove("url");
                }
                self.set_cookie(view, cookie);
                Ok(json!({ "success": true }).to_string())
            }
            "Network.deleteCookies" => {
                let session = self.session_of(view);
                if let Some(jar) = self.jars.get_mut(&session) {
                    jar.retain(|cookie| {
                        (&cookie["name"], &cookie["domain"], &cookie["path"])
                            != (&params["name"], &params["domain"], &params["path"])
                    });
                }
                Ok("{}".to_owned())
            }
            other => panic!("the plugin sent {other}"),
        }
    }

    fn fetch(&mut self, _request: &HttpRequest) -> Result<HttpResponse, Failure> {
        Err(Capability::Http.denied())
    }

    fn project_read(&mut self, path: &str) -> Result<Vec<u8>, Failure> {
        self.project
            .get(path)
            .cloned()
            .ok_or_else(|| Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn project_list(&mut self, dir: &str) -> Result<Vec<String>, Failure> {
        let protected = [".rbrowser/flows", ".rbrowser/snippets", ".rbrowser/plugins"];
        if !protected.contains(&dir) {
            return Err(Capability::ProjectRead.denied());
        }
        if self.untrusted {
            return Err(Failure::coded(
                FailureKind::AccessDenied,
                codes::UNTRUSTED_WORKSPACE,
            ));
        }
        Err(Failure::coded(FailureKind::NotFound, codes::NOT_FOUND))
    }

    fn out_write(&mut self, path: &str, bytes: &[u8]) -> Result<(), Failure> {
        self.project.insert(
            format!(".rbrowser/out/storage-panel/{path}"),
            bytes.to_vec(),
        );
        Ok(())
    }

    fn report_findings(&mut self, _items: &[Finding]) -> Result<(), Failure> {
        Err(Capability::Findings.denied())
    }

    fn sidecar_call(&mut self, _method: &str, _params: &str) -> Result<String, Failure> {
        Err(Capability::Sidecar.denied())
    }
}
