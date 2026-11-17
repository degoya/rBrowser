//! A backend for tests, of this crate and of the host's: views in memory, commands recorded, a
//! navigation that settles at once. Not used by the app.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rb_util::lock;
use std::time::Duration;

use async_trait::async_trait;
use rb_commands::CommandError;
use rb_engine::{EngineKind, Target, ViewId, ViewInfo};
use serde_json::{Value, json};

use crate::backend::{Backend, ConfigScope, McpOptions, SettingScope, ToolFailure};
use crate::logs::LogStore;

/// The keys the fake refuses, as the app refuses its `x-protected` ones.
pub const PROTECTED: &[&str] = &["mcp.enabled", "mcp.allowEvaluate", "plugins.enabled"];

/// A view for a test: id, breakpoint, URL.
#[must_use]
pub fn view(id: u32, breakpoint: &str, url: &str) -> ViewInfo {
    ViewInfo {
        id: ViewId(id),
        label: breakpoint.to_owned(),
        breakpoint: Some(breakpoint.to_owned()),
        cell_breakpoint: Some(breakpoint.to_owned()),
        engine: EngineKind::Chromium,
        width: 375,
        height: 812,
        dpr: 3.0,
        scale: 1.0,
        url: url.to_owned(),
        title: String::new(),
        loading: false,
        session: "default".to_owned(),
        cell: None,
    }
}

/// A command error the fake keeps (`CommandError` is not `Clone`).
#[derive(Clone, Debug)]
enum Refusal {
    NotImplemented,
    InvalidArgs(String),
    Failed(String),
}

/// The fake.
#[derive(Debug)]
pub struct FakeBackend {
    views: Mutex<Vec<ViewInfo>>,
    calls: Mutex<Vec<(String, Value)>>,
    answers: Mutex<BTreeMap<String, Result<Value, Refusal>>>,
    settings: Mutex<BTreeMap<String, Value>>,
    options: Mutex<McpOptions>,
    cdp: Mutex<Vec<(u32, String, Value)>>,
    cdp_answers: Mutex<BTreeMap<String, Value>>,
    documents: Mutex<BTreeMap<String, Value>>,
    logs: Arc<LogStore>,
    cdp_endpoint: Mutex<Option<Value>>,
    cdp_endpoint_views: Mutex<Vec<Option<u32>>>,
    flows: Mutex<Option<Value>>,
    flow_runs: Mutex<Vec<FlowRun>>,
    plugin_tools: Mutex<Vec<crate::tools::plugins::PluginToolInfo>>,
}

/// The fake records a host tool `name` as the command `host:<name>` ([`FakeBackend::calls`])
/// and answers what [`FakeBackend::answer`] set for that id.
pub const HOST_TOOL_PREFIX: &str = "host:";
/// One `flow_run` the fake was asked for: name, views, wait.
pub type FlowRun = (String, Option<Vec<ViewId>>, bool);

/// What the fake's mask replaces: any string holding this.
pub const SECRET_MARK: &str = "SECRET";

impl Default for FakeBackend {
    fn default() -> Self {
        Self::new(vec![
            view(1, "mobile", "about:blank"),
            view(2, "desktop", "about:blank"),
        ])
    }
}

impl FakeBackend {
    /// A fake with `views`.
    #[must_use]
    pub fn new(views: Vec<ViewInfo>) -> Self {
        Self {
            views: Mutex::new(views),
            calls: Mutex::new(Vec::new()),
            answers: Mutex::new(BTreeMap::new()),
            settings: Mutex::new(BTreeMap::new()),
            options: Mutex::new(McpOptions {
                navigation_timeout: Duration::from_millis(500),
                allow_evaluate: false,
                max_answer_bytes: crate::backend::DEFAULT_MAX_ANSWER_BYTES,
            }),
            cdp: Mutex::new(Vec::new()),
            cdp_answers: Mutex::new(BTreeMap::new()),
            documents: Mutex::new(BTreeMap::new()),
            logs: Arc::new(LogStore::default()),
            cdp_endpoint: Mutex::new(None),
            cdp_endpoint_views: Mutex::new(Vec::new()),
            flows: Mutex::new(None),
            flow_runs: Mutex::new(Vec::new()),
            plugin_tools: Mutex::new(Vec::new()),
        }
    }

    /// The plugin tools the fake lists from now on (RB-060-12).
    pub fn set_plugin_tools(&self, tools: Vec<crate::tools::plugins::PluginToolInfo>) {
        *lock(&self.plugin_tools) = tools;
    }

    /// The CDP commands sent so far: view, method, params.
    #[must_use]
    pub fn cdp_calls(&self) -> Vec<(u32, String, Value)> {
        lock(&self.cdp).clone()
    }

    /// Answers the CDP command `method` with `answer` from now on (`{}` otherwise).
    pub fn answer_cdp(&self, method: &str, answer: Value) {
        lock(&self.cdp_answers).insert(method.to_owned(), answer);
    }

    /// The project file `scope` as the fake holds it.
    #[must_use]
    pub fn document(&self, scope: ConfigScope) -> Value {
        lock(&self.documents)
            .get(scope_name(scope))
            .cloned()
            .unwrap_or_else(|| json!({}))
    }

    /// Sets the project file `scope`.
    pub fn set_document(&self, scope: ConfigScope, document: Value) {
        lock(&self.documents).insert(scope_name(scope).to_owned(), document);
    }

    /// The commands run so far: id and arguments.
    #[must_use]
    pub fn calls(&self) -> Vec<(String, Value)> {
        lock(&self.calls).clone()
    }

    /// Answers the command `id` with `answer` from now on.
    pub fn answer(&self, id: &str, answer: Result<Value, CommandError>) {
        let answer = answer.map_err(|error| match error {
            CommandError::NotImplemented => Refusal::NotImplemented,
            CommandError::InvalidArgs(text) => Refusal::InvalidArgs(text),
            other => Refusal::Failed(other.to_string()),
        });
        lock(&self.answers).insert(id.to_owned(), answer);
    }

    /// The settings written.
    #[must_use]
    pub fn written(&self) -> BTreeMap<String, Value> {
        lock(&self.settings).clone()
    }

    /// The URL of `view` now.
    #[must_use]
    pub fn url_of(&self, view: u32) -> Option<String> {
        lock(&self.views)
            .iter()
            .find(|info| info.id == ViewId(view))
            .map(|info| info.url.clone())
    }

    /// Marks `view` as loading or not.
    pub fn set_loading(&self, view: u32, loading: bool) {
        for info in lock(&self.views).iter_mut() {
            if info.id == ViewId(view) {
                info.loading = loading;
            }
        }
    }

    /// The CDP proxy is on with this endpoint answer (`None`: off).
    pub fn set_cdp_endpoint(&self, answer: Option<Value>) {
        *lock(&self.cdp_endpoint) = answer;
    }

    /// The views `cdp_endpoint` was asked for so far.
    #[must_use]
    pub fn cdp_endpoint_views(&self) -> Vec<Option<u32>> {
        lock(&self.cdp_endpoint_views).clone()
    }

    /// The workspace is trusted and `flow_list` answers `answer` (`flow_run` too); untrusted
    /// until called.
    pub fn set_flows(&self, answer: Value) {
        *lock(&self.flows) = Some(answer);
    }

    /// The runs `flow_run` was asked for so far.
    #[must_use]
    pub fn flow_runs(&self) -> Vec<FlowRun> {
        lock(&self.flow_runs).clone()
    }

    /// Sets the settings of the server.
    pub fn set_options(&self, options: McpOptions) {
        *lock(&self.options) = options;
    }

    fn targets(&self, args: &Value) -> Vec<ViewId> {
        let target: Target = args
            .get("target")
            .cloned()
            .and_then(|target| serde_json::from_value(target).ok())
            .unwrap_or(Target::All);
        lock(&self.views)
            .iter()
            .map(|view| view.id)
            .filter(|id| target == Target::All || target == Target::View(*id))
            .collect()
    }
}

#[async_trait]
impl Backend for FakeBackend {
    fn run_command(&self, id: &str, args: Value) -> Result<Value, CommandError> {
        lock(&self.calls).push((id.to_owned(), args.clone()));
        if let Some(answer) = lock(&self.answers).get(id) {
            return answer.clone().map_err(|refusal| match refusal {
                Refusal::NotImplemented => CommandError::NotImplemented,
                Refusal::InvalidArgs(text) => CommandError::InvalidArgs(text),
                Refusal::Failed(text) => CommandError::Failed(text),
            });
        }
        if id == "view.navigate"
            && let Some(url) = args.get("url").and_then(Value::as_str)
        {
            let targets = self.targets(&args);
            for view in lock(&self.views).iter_mut() {
                if targets.contains(&view.id) {
                    view.url = url.to_owned();
                }
            }
        }
        Ok(Value::Null)
    }

    fn views(&self) -> Vec<ViewInfo> {
        lock(&self.views).clone()
    }

    fn plugin_tools(&self) -> Vec<crate::tools::plugins::PluginToolInfo> {
        lock(&self.plugin_tools).clone()
    }

    async fn host_tool(&self, name: &str, args: Value) -> Result<Value, ToolFailure> {
        let id = format!("{HOST_TOOL_PREFIX}{name}");
        self.run_command(&id, args).map_err(ToolFailure::from)
    }

    fn app_info(&self) -> Value {
        json!({ "name": "rBrowser", "version": "0.0.0-test" })
    }

    fn project_info(&self) -> Value {
        json!({ "root": "/tmp/site", "trusted": false })
    }

    fn config(&self) -> Value {
        json!({ "version": 1, "startUrl": "https://a.test/" })
    }

    fn settings(&self) -> Value {
        settings_document(&lock(&self.settings))
    }

    fn set_setting(
        &self,
        key: &str,
        value: &Value,
        _scope: SettingScope,
    ) -> Result<(), ToolFailure> {
        if PROTECTED.contains(&key) {
            return Err(ToolFailure::new(
                "settings.rejected.protected",
                format!("`{key}` is protected: only a person can change it in the settings"),
            )
            .with_setting(key));
        }
        lock(&self.settings).insert(key.to_owned(), value.clone());
        Ok(())
    }

    fn options(&self) -> McpOptions {
        *lock(&self.options)
    }

    async fn wsl_info(&self) -> Value {
        json!({ "distros": [], "helper": "stopped" })
    }

    async fn cdp(&self, view: ViewId, method: &str, params: Value) -> Result<Value, ToolFailure> {
        if !lock(&self.views).iter().any(|info| info.id == view) {
            return Err(ToolFailure::new(
                "view_not_found",
                format!("unknown view {view}"),
            ));
        }
        lock(&self.cdp).push((view.0, method.to_owned(), params));
        Ok(lock(&self.cdp_answers)
            .get(method)
            .cloned()
            .unwrap_or_else(|| json!({})))
    }

    fn mask(&self, value: &Value) -> Value {
        match value {
            Value::String(text) if text.contains(SECRET_MARK) => Value::String("***".to_owned()),
            Value::Array(items) => Value::Array(items.iter().map(|item| self.mask(item)).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(key, child)| (key.clone(), self.mask(child)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    fn logs(&self) -> Arc<LogStore> {
        Arc::clone(&self.logs)
    }

    fn config_document(&self, scope: ConfigScope) -> Result<Value, ToolFailure> {
        Ok(self.document(scope))
    }

    async fn cdp_endpoint(&self, view: Option<ViewId>) -> Result<Value, ToolFailure> {
        lock(&self.cdp_endpoint_views).push(view.map(|view| view.0));
        lock(&self.cdp_endpoint).clone().ok_or_else(|| {
            ToolFailure::new("cdp.disabled", "the CDP proxy is off").with_setting("cdp.enabled")
        })
    }

    async fn flow_list(&self) -> Result<Value, ToolFailure> {
        lock(&self.flows).clone().ok_or_else(untrusted)
    }

    async fn flow_run(
        &self,
        name: &str,
        views: Option<Vec<ViewId>>,
        wait: bool,
    ) -> Result<Value, ToolFailure> {
        let answer = lock(&self.flows).clone().ok_or_else(untrusted)?;
        lock(&self.flow_runs).push((name.to_owned(), views, wait));
        Ok(answer)
    }

    fn write_config_document(
        &self,
        scope: ConfigScope,
        document: &Value,
    ) -> Result<(), ToolFailure> {
        // The app runs `rb_config::check`; the fake refuses a negative width the same way.
        let negative = document["breakpoints"].as_array().is_some_and(|list| {
            list.iter()
                .any(|b| b["width"].as_i64().is_some_and(|w| w < 0))
        });
        if negative {
            return Err(ToolFailure::new(
                "config.error.invalid",
                "the config does not validate",
            ));
        }
        self.set_document(scope, document.clone());
        Ok(())
    }
}

/// The keys of the fake's settings: key, category, section, default, label.
const FAKE_KEYS: &[(&str, &str, &str, bool, &str)] = &[
    (
        "startup.openLastProject",
        "general",
        "startup",
        false,
        "Open the last project",
    ),
    ("grid.fitToWindow", "layout", "zoom", true, "Fit to window"),
    (
        "mcp.allowEvaluate",
        "security",
        "evaluate",
        false,
        "Allow evaluate",
    ),
];

/// The settings document as the app hands it to `settings_get` (`crate::settings_view`): three
/// categories with a section each, the [`FAKE_KEYS`] and every other key `written`, with the
/// written values (origin `user`).
#[must_use]
pub fn settings_document(written: &BTreeMap<String, Value>) -> Value {
    let mut keys: Vec<Value> = FAKE_KEYS
        .iter()
        .map(|(key, category, section, default, label)| {
            let (value, origin) = written
                .get(*key)
                .map_or((Value::Bool(*default), "default"), |value| {
                    (value.clone(), "user")
                });
            json!({
                "key": key, "category": category, "section": section, "type": "boolean",
                "value": value, "origin": origin, "default": default, "label": label,
                "description": "", "protected": PROTECTED.contains(key),
            })
        })
        .collect();
    keys.extend(
        written
            .iter()
            .filter(|(key, _)| !FAKE_KEYS.iter().any(|known| known.0 == key.as_str()))
            .map(|(key, value)| {
                json!({ "key": key, "category": "general", "section": "startup",
                        "value": value, "origin": "user" })
            }),
    );
    json!({
        "categories": [
            { "id": "general", "title": "General", "sections": [{ "id": "startup", "title": "Startup" }] },
            { "id": "layout", "title": "Layout", "sections": [{ "id": "zoom", "title": "Zoom" }] },
            { "id": "security", "title": "Security", "sections": [{ "id": "evaluate", "title": "Evaluate" }] },
        ],
        "keys": keys,
    })
}

fn untrusted() -> ToolFailure {
    ToolFailure::new(
        "flows.untrusted",
        "the flows of this project are blocked until its workspace is trusted",
    )
}

fn scope_name(scope: ConfigScope) -> &'static str {
    match scope {
        ConfigScope::Project => "project",
        ConfigScope::Local => "local",
    }
}

/// A tool's answer as `(is_error, the JSON of its first text block)`.
#[must_use]
pub fn answer_json(result: &rmcp::model::CallToolResult) -> (bool, Value) {
    let wire = serde_json::to_value(result).unwrap_or(Value::Null);
    let is_error = wire["isError"].as_bool().unwrap_or(false);
    let text = wire["content"][0]["text"].as_str().unwrap_or("null");
    (is_error, serde_json::from_str(text).unwrap_or(Value::Null))
}

/// A server over `fake`.
#[must_use]
pub fn server(fake: &std::sync::Arc<FakeBackend>) -> crate::RbMcpServer {
    let backend: std::sync::Arc<dyn Backend> = fake.clone();
    crate::RbMcpServer::new(backend)
}
