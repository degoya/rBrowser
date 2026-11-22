//! What the tools act on: the app behind the server. `crates/rbrowser` implements [`Backend`]
//! over its core - every action through the one command registry (toolbar, palette, shortcuts
//! and MCP run the same commands), the reads straight from the host - and the tests implement it
//! with a fake ([`crate::testing`]).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rb_commands::CommandError;
use rb_engine::{ViewId, ViewInfo};
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::logs::LogStore;

/// The default of `mcp.navigationTimeoutMs`.
pub const DEFAULT_NAVIGATION_TIMEOUT: Duration = Duration::from_secs(15);

/// Where `settings_set` writes.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum SettingScope {
    /// The user's settings file.
    #[default]
    User,
    /// `.rbrowser/config.json`, checked in.
    Project,
    /// `.rbrowser/config.local.json`, this machine only.
    Local,
}

/// The default of `mcp.maxAnswerBytes`.
pub const DEFAULT_MAX_ANSWER_BYTES: usize = 256 * 1024;

/// Which project file `config_set` and the breakpoint tools write.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ConfigScope {
    /// `.rbrowser/config.json`, checked in.
    #[default]
    Project,
    /// `.rbrowser/config.local.json`, this machine only.
    Local,
}

/// The settings the server reads at every call (they change at runtime).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McpOptions {
    /// `mcp.navigationTimeoutMs`: how long `navigate`, `reload`, `back` and `forward` wait for
    /// the final URL.
    pub navigation_timeout: Duration,
    /// `mcp.allowEvaluate`: page JavaScript over MCP (the `evaluate` tool, off by default).
    pub allow_evaluate: bool,
    /// `mcp.maxAnswerBytes`: the most bytes `evaluate` and `snapshot` answer; more is cut.
    pub max_answer_bytes: usize,
    /// `mcp.prompts`: the prompts are listed and answered (RB-095-01).
    pub prompts: bool,
    /// `mcp.resources`: the resources are listed and read (RB-095-01).
    pub resources: bool,
    /// `mcp.scripts.*`: MCP code mode, `script_run` (RB-098-01).
    pub scripts: crate::scripts::ScriptOptions,
}

impl Default for McpOptions {
    fn default() -> Self {
        Self {
            navigation_timeout: DEFAULT_NAVIGATION_TIMEOUT,
            allow_evaluate: false,
            max_answer_bytes: DEFAULT_MAX_ANSWER_BYTES,
            prompts: true,
            resources: true,
            scripts: crate::scripts::ScriptOptions::default(),
        }
    }
}

/// Why a tool failed: a stable code, an English message and, where a setting decides, its key
/// (the deep link into the settings).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolFailure {
    /// The stable code (`view_not_found`, `settings.rejected.protected`, ...).
    pub code: String,
    /// The English message.
    pub message: String,
    /// The setting that decides it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings_key: Option<String>,
}

impl ToolFailure {
    /// A failure with `code` and `message`.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            settings_key: None,
        }
    }

    /// The same failure, pointing at the setting `key`.
    #[must_use]
    pub fn with_setting(mut self, key: &str) -> Self {
        self.settings_key = Some(key.to_owned());
        self
    }
}

impl From<CommandError> for ToolFailure {
    fn from(error: CommandError) -> Self {
        let code = match &error {
            CommandError::NotImplemented => "not_implemented",
            CommandError::UnknownCommand(_) | CommandError::InvalidId(_) => "unknown_command",
            CommandError::InvalidArgs(_) => "invalid_arguments",
            _ => "command_failed",
        };
        Self::new(code, error.to_string())
    }
}

/// The app behind the server. The synchronous methods may block briefly (a command that writes
/// a file); the server calls them off the async runtime's workers.
#[async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Runs the command `id` of the one command registry with `args`.
    fn run_command(&self, id: &str, args: Value) -> Result<Value, CommandError>;

    /// Every view, in grid order.
    fn views(&self) -> Vec<ViewInfo>;

    /// What `views_list` answers: [`Backend::views`] and per view what the host adds to it (its
    /// desync, whether its navigation is synced). The views alone by default.
    fn views_list(&self) -> Value {
        serde_json::to_value(self.views()).unwrap_or(Value::Null)
    }

    /// Runs the tool `name` the host implements itself - sync, input replay, scroll, the page
    /// API's bus and the screenshot ([`crate::tools::sync`], [`crate::tools::capture`]) - with
    /// its arguments, `target` already `"all"` or a view id.
    async fn host_tool(&self, name: &str, args: Value) -> Result<Value, ToolFailure>;

    /// The loaded plugins' MCP tools, `<mcp_prefix>_<tool>` (RB-060-12); [`Backend::host_tool`]
    /// runs them. None by default.
    fn plugin_tools(&self) -> Vec<crate::tools::plugins::PluginToolInfo> {
        Vec::new()
    }

    /// The facts of Settings > About (`about::app_info`).
    fn app_info(&self) -> Value;

    /// The open project: root, config file, trust state (read only).
    fn project_info(&self) -> Value;

    /// The effective project config, secrets masked.
    fn config(&self) -> Value;

    /// Every setting with its effective value and origin, secrets masked.
    fn settings(&self) -> Value;

    /// Writes `value` (`null` resets) to the setting `key` in `scope`, as an MCP caller: an
    /// `x-protected` key is refused.
    fn set_setting(&self, key: &str, value: &Value, scope: SettingScope)
    -> Result<(), ToolFailure>;

    /// The settings of the server in effect now.
    fn options(&self) -> McpOptions;

    /// The WSL state (`rb_wsl::info::WslInfo`).
    async fn wsl_info(&self) -> Value;

    /// Sends the CDP command `method` to `view`'s page.
    async fn cdp(&self, view: ViewId, method: &str, params: Value) -> Result<Value, ToolFailure>;

    /// `value` with its secrets masked (the host's log mask: known tokens, credentials by their
    /// shape, header values).
    fn mask(&self, value: &Value) -> Value;

    /// The console, network and error buffers of the views.
    fn logs(&self) -> Arc<LogStore>;

    /// The project file `scope` as it is on disk (an empty object when it does not exist).
    fn config_document(&self, scope: ConfigScope) -> Result<Value, ToolFailure>;

    /// The dotted keys of the installed plugins' settings that are protected from MCP
    /// (`x-protected`: allowed hosts, secrets), whether a plugin is loaded or not, and the whole
    /// prefix of a package whose keys do not read (RB-094-31); `config_set` refuses them, also
    /// a parent write that would remove or change one (`config_edit::set`). The core's are in
    /// `config_edit`'s own lists.
    fn mcp_protected_keys(&self) -> Vec<String>;

    /// Writes `document` as the project file `scope` after the checks the settings window
    /// runs (migrated, merged with the other file, `rb_config::check`).
    fn write_config_document(
        &self,
        scope: ConfigScope,
        document: &Value,
    ) -> Result<(), ToolFailure>;

    /// The CDP proxy's endpoint (RB-030-06): its URLs with the token, the page socket of
    /// `view`, the route from WSL. Only an authenticated client reaches a tool, so the token
    /// may be in the answer. While the proxy is off: a failure naming `cdp.enabled`.
    async fn cdp_endpoint(&self, view: Option<ViewId>) -> Result<Value, ToolFailure> {
        let _ = view;
        Err(
            ToolFailure::new("cdp.disabled", "this rBrowser has no CDP proxy")
                .with_setting("cdp.enabled"),
        )
    }

    /// The project's flows (RB-040-07): the folder, whether the workspace is trusted, every
    /// flow with its head comments, the run going on and the last one.
    async fn flow_list(&self) -> Result<Value, ToolFailure> {
        Err(no_flows())
    }

    /// Runs the flow `name` in `views` (`None`: the flow's own, else all); `wait` answers the
    /// result, otherwise the run's id at once. An untrusted workspace is refused
    /// (`flows.untrusted`); no tool grants trust.
    async fn flow_run(
        &self,
        name: &str,
        views: Option<Vec<ViewId>>,
        wait: bool,
    ) -> Result<Value, ToolFailure> {
        let _ = (name, views, wait);
        Err(no_flows())
    }

    /// The sandbox `script_run` runs scripts in (RB-098-01, `crates/rb-script` in the app);
    /// `None` - the default - answers `script.unavailable`.
    fn script_runner(&self) -> Option<Arc<dyn crate::scripts::ScriptRunner>> {
        None
    }

    /// The last check run (RB-060-01) as `{ run, messages }`: the run's `findings.json` and the
    /// texts of its message keys (`rbrowser://findings/latest`, RB-095-01). `None` before the
    /// first run, and by default.
    fn latest_findings(&self) -> Option<Value> {
        None
    }

    /// The last screenshots saved below the project's screenshot folder (RB-050-03) as
    /// `{ folder, shots, overview }`, each shot `{ view, label, width, height, path, error }`
    /// and never the image (`rbrowser://screenshots/latest`, RB-095-01). `None` before the
    /// first, and by default.
    fn latest_screenshots(&self) -> Option<Value> {
        None
    }

    /// The PNG of file `index` of [`Backend::latest_screenshots`] - the shots with a `path` in
    /// their order, the overview last - when it has at most `max_bytes`; refused with
    /// `answer_too_large` over it. Only those files: the index never names a path of its own.
    fn latest_screenshot_png(
        &self,
        index: usize,
        max_bytes: usize,
    ) -> Result<Vec<u8>, ToolFailure> {
        let _ = (index, max_bytes);
        Err(ToolFailure::new(
            "screenshots_none",
            "no screenshot was saved yet: take one with screenshot",
        ))
    }
}

/// The failure of a backend without flows.
fn no_flows() -> ToolFailure {
    ToolFailure::new("not_implemented", "this rBrowser runs no flows")
}
