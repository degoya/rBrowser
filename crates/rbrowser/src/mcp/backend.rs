//! [`rb_mcp::Backend`] over the core: every action a command of the one registry, the reads from
//! the host, the settings window's write path with the MCP caller (an `x-protected` key is
//! refused there; shortcuts go through the shortcut editor's path, [`super::shortcuts`]), every
//! change audited ([`super::settings::record`]), and every answer that can carry a secret masked
//! by rb-diag.

use std::sync::{Arc, Weak};
use std::time::Duration;

use async_trait::async_trait;
use rb_commands::CommandError;
use rb_config::ConfigFile;
use rb_engine::{ViewHost as _, ViewId, ViewInfo};
use rb_ipc::CoreToShell;
use rb_mcp::{Backend, ConfigScope, LogLimits, LogStore, McpOptions, SettingScope, ToolFailure};
use rb_settings::Scope;
use serde_json::{Value, json};

use crate::control::Core;
use crate::settings_window::Caller;

/// The tools' window into the core; holds it weakly, so the server never keeps a closed window
/// alive.
pub struct CoreBackend {
    core: Weak<Core>,
    logs: Arc<LogStore>,
}

impl CoreBackend {
    /// The backend over `core`, its log buffers `logs`.
    pub fn new(core: &Arc<Core>, logs: Arc<LogStore>) -> Self {
        Self {
            core: Arc::downgrade(core),
            logs,
        }
    }

    fn core(&self) -> Option<Arc<Core>> {
        self.core.upgrade()
    }
}

/// `value` with its secrets masked (header values, credentials in addresses, tokens).
pub fn masked(value: &Value) -> Value {
    rb_diag::redact::redact_json(value).value
}

/// The settings window's scope for `scope`.
pub fn scope_of(scope: SettingScope) -> Scope {
    match scope {
        SettingScope::User => Scope::User,
        SettingScope::Project => Scope::Project,
        SettingScope::Local => Scope::Local,
    }
}

/// A refused `setSetting` as a tool failure: the first reason's i18n key is the code.
pub fn rejection(key: &str, answer: &CoreToShell) -> Option<ToolFailure> {
    let CoreToShell::SettingRejected { issues, .. } = answer else {
        return None;
    };
    let reasons: Vec<&str> = issues
        .iter()
        .map(|issue| issue.message_key.as_str())
        .collect();
    let code = reasons
        .first()
        .copied()
        .unwrap_or("settings.rejected.invalid");
    let message = if code == crate::settings_window::write::reason::PROTECTED {
        format!("`{key}` is protected: only a person can change it in rBrowser's settings")
    } else {
        format!("the setting `{key}` was refused: {}", reasons.join(", "))
    };
    Some(ToolFailure::new(code, message).with_setting(key))
}

/// The buffer sizes of `settings`.
pub fn log_limits(settings: &rb_config::McpSettings) -> LogLimits {
    let size = |value: u32| usize::try_from(value).unwrap_or(usize::MAX);
    LogLimits {
        console: size(settings.console_log_limit),
        network: size(settings.network_request_limit),
        errors: size(settings.error_limit),
    }
}

/// The project file of `scope`.
pub fn file_of(scope: ConfigScope) -> ConfigFile {
    match scope {
        ConfigScope::Project => ConfigFile::Project,
        ConfigScope::Local => ConfigFile::Local,
    }
}

/// A refused config check as a tool failure: the first reason's key is the code, every
/// problem with its JSON pointer in the message.
pub fn check_failure(issues: &[rb_ipc::SettingIssue]) -> ToolFailure {
    let code = issues
        .first()
        .map_or("config.error.invalid", |issue| issue.message_key.as_str());
    let problems: Vec<String> = issues
        .iter()
        .map(|issue| format!("{} {}", issue.path, issue.message_key))
        .collect();
    ToolFailure::new(
        code,
        format!(
            "the config does not pass its check: {}",
            problems.join("; ")
        ),
    )
}

fn no_project() -> ToolFailure {
    ToolFailure::new("settings.rejected.no_project", "no project is open")
}

/// The file at `path` as JSON; an empty object when it does not exist.
fn read_or_empty(path: &std::path::Path) -> Result<Value, ToolFailure> {
    if !path.is_file() {
        return Ok(json!({}));
    }
    rb_config::read_json(path)
        .map_err(|error| ToolFailure::new("config.error.parse", error.to_string()))
}

fn closed() -> CommandError {
    CommandError::Failed("the window is closed".to_owned())
}

#[async_trait]
impl Backend for CoreBackend {
    fn run_command(&self, id: &str, args: Value) -> Result<Value, CommandError> {
        let core = self.core().ok_or_else(closed)?;
        // MCP's `navigate`, `checks_run` and `breakpoints_detect` load http(s) and the project's
        // own files only (RB-094-10).
        super::url_policy::check_command(id, &args, core.host().project_root().as_deref())?;
        // Adding a portal asset origin writes the project config: a person's click (RB-090-71).
        crate::portal::offer::refuse_mcp(id)?;
        // Installing an update is a person's click; `update_install` asks them (RB-090-14).
        crate::updates::refuse_mcp(id)?;
        // The setup wizard is a person's dialog (RB-091-05).
        crate::first_run::refuse_mcp(id)?;
        // A restart ends every MCP session (RB-094-23).
        crate::restart::refuse_mcp(id)?;
        // Basic auth credentials and login state are a person's (RB-094-08).
        crate::basic_auth::refuse_mcp(id)?;
        crate::session_state::refuse_mcp(id)?;
        // Installing a plugin is a person's (RB-094-13).
        crate::plugins::bundle_install::refuse_mcp(id)?;
        core.run_command(id, args)
    }

    fn views(&self) -> Vec<ViewInfo> {
        self.core()
            .map(|core| core.host().views())
            .unwrap_or_default()
    }

    fn views_list(&self) -> Value {
        self.core()
            .map_or_else(|| json!([]), |core| super::host_tools::views_list(&core))
    }

    async fn host_tool(&self, name: &str, args: Value) -> Result<Value, ToolFailure> {
        let core = self
            .core()
            .ok_or_else(|| ToolFailure::new("closed", "the window is closed"))?;
        super::host_tools::call(&core, name, args).await
    }

    fn plugin_tools(&self) -> Vec<rb_mcp::PluginToolInfo> {
        self.core()
            .map(|core| crate::plugins::mcp::tools(&core))
            .unwrap_or_default()
    }

    fn app_info(&self) -> Value {
        self.core().map_or_else(crate::about::app_info, |core| {
            crate::about::app_info_of(&core)
        })
    }

    fn project_info(&self) -> Value {
        let Some(core) = self.core() else {
            return Value::Null;
        };
        let host = core.host();
        let root = host.project_root();
        let state = host.shell_state();
        json!({
            "root": root.as_ref().map(|root| root.to_string_lossy().into_owned()),
            "configFile": root.as_ref().map(|root| root
                .join(rb_config::CONFIG_DIR)
                .join(rb_config::CONFIG_FILE)
                .to_string_lossy()
                .into_owned()),
            "trusted": state.trusted,
            "environment": state.environment,
            "preset": state.preset,
            // RB-030-05: the hot reload watcher; `null` before it started.
            "watcher": crate::reload::watcher_info(),
        })
    }

    fn config(&self) -> Value {
        self.core()
            .and_then(|core| serde_json::to_value(core.host().config()).ok())
            .map_or(Value::Null, |config| masked(&config))
    }

    fn settings(&self) -> Value {
        self.core()
            .map_or(Value::Null, |core| super::settings::read(&core))
    }

    fn set_setting(
        &self,
        key: &str,
        value: &Value,
        scope: SettingScope,
    ) -> Result<(), ToolFailure> {
        let core = self
            .core()
            .ok_or_else(|| ToolFailure::new("closed", "the window is closed"))?;
        let shortcut = super::shortcuts::is_shortcut_key(key);
        // A shortcut's value lives in `shortcuts.bindings`, one key for every command.
        let audited = if shortcut {
            crate::settings_window::SHORTCUTS_KEY
        } else {
            key
        };
        let old = super::settings::current(&core, audited);
        if shortcut {
            super::shortcuts::set(&core, key, value, scope)?;
        } else {
            let answer = crate::settings_window::set_setting(
                &core,
                key,
                value,
                scope_of(scope),
                Caller::Mcp,
            );
            if let Some(failure) = rejection(key, &answer) {
                return Err(failure);
            }
        }
        let new = super::settings::current(&core, audited);
        super::settings::record(&core, key, scope, &old, &new);
        Ok(())
    }

    fn options(&self) -> McpOptions {
        self.core().map_or_else(McpOptions::default, |core| {
            let settings = core.user_settings().mcp;
            McpOptions {
                navigation_timeout: Duration::from_millis(u64::from(
                    settings.navigation_timeout_ms,
                )),
                allow_evaluate: settings.allow_evaluate,
                max_answer_bytes: usize::try_from(settings.max_answer_bytes)
                    .unwrap_or(rb_mcp::backend::DEFAULT_MAX_ANSWER_BYTES),
                prompts: settings.prompts,
                resources: settings.resources,
                scripts: rb_mcp::ScriptOptions {
                    enabled: settings.scripts.enabled,
                    timeout: Duration::from_millis(u64::from(settings.scripts.timeout_ms)),
                    max_calls: settings.scripts.max_calls,
                    max_result_bytes: usize::try_from(settings.scripts.max_result_bytes)
                        .unwrap_or(usize::MAX),
                },
            }
        })
    }

    fn script_runner(&self) -> Option<Arc<dyn rb_mcp::ScriptRunner>> {
        Some(super::scripts::runner())
    }

    async fn wsl_info(&self) -> Value {
        let user = self
            .core()
            .map(|core| core.user_settings())
            .unwrap_or_default();
        super::wsl::info(&user).await
    }

    async fn cdp(&self, view: ViewId, method: &str, params: Value) -> Result<Value, ToolFailure> {
        let core = self
            .core()
            .ok_or_else(|| ToolFailure::new("closed", "the window is closed"))?;
        core.host()
            .cdp_send(view, method, params)
            .await
            .map_err(|error| ToolFailure::new("cdp_failed", masked_text(&error.to_string())))
    }

    fn mask(&self, value: &Value) -> Value {
        masked(value)
    }

    fn logs(&self) -> Arc<LogStore> {
        Arc::clone(&self.logs)
    }

    fn config_document(&self, scope: ConfigScope) -> Result<Value, ToolFailure> {
        let root = self
            .core()
            .and_then(|core| core.host().project_root())
            .ok_or_else(no_project)?;
        read_or_empty(&file_of(scope).path(&root))
    }

    fn mcp_protected_keys(&self) -> Vec<String> {
        let Some(core) = self.core() else {
            return Vec::new();
        };
        let registry = core.settings_registry();
        let loaded = registry
            .iter()
            .filter(|def| def.protected_from_mcp && registry.plugin_of(&def.key).is_some())
            .map(|def| def.key.clone());
        // Every installed package's too, loaded or not (RB-094-31): the folders the load reads.
        let installed = rb_plugin_host::Sources {
            bundled: crate::analyzer::plugins::bundled_dir(),
            user: crate::analyzer::plugins::user_dir(),
            project_root: core.host().project_root(),
            ..rb_plugin_host::Sources::default()
        };
        let mut keys = super::installed_keys::protected_keys(&installed);
        keys.extend(loaded);
        keys.into_iter().collect()
    }

    fn write_config_document(
        &self,
        scope: ConfigScope,
        document: &Value,
    ) -> Result<(), ToolFailure> {
        let root = self
            .core()
            .and_then(|core| core.host().project_root())
            .ok_or_else(no_project)?;
        let file = file_of(scope);
        let path = file.path(&root);
        if !crate::settings_window::user_file::writable(&path) {
            return Err(ToolFailure::new(
                "settings.rejected.not_saved",
                "the config file cannot be read, so it is not written over",
            ));
        }
        let (project, local) = match file {
            ConfigFile::Project => (
                document.clone(),
                Some(read_or_empty(&ConfigFile::Local.path(&root))?),
            ),
            ConfigFile::Local => (
                read_or_empty(&ConfigFile::Project.path(&root))?,
                Some(document.clone()),
            ),
        };
        crate::settings_window::write::validate_project(&project, local.as_ref(), "")
            .map_err(|issues| check_failure(&issues))?;
        rb_config::write_config_file(&root, file, document)
            .map_err(|error| ToolFailure::new("settings.rejected.not_saved", error.to_string()))?;
        // In effect at once, with or without the watcher (RB-094-23).
        if let Some(core) = self.core() {
            crate::project_reload::after_write(&core);
        }
        Ok(())
    }

    async fn cdp_endpoint(&self, view: Option<ViewId>) -> Result<Value, ToolFailure> {
        let core = self
            .core()
            .ok_or_else(|| ToolFailure::new("closed", "the window is closed"))?;
        let server =
            crate::cdp_proxy::current().ok_or_else(crate::cdp_proxy::endpoint::disabled)?;
        let page = match view {
            None => None,
            Some(view) => {
                let target = core.host().target_id(view).ok_or_else(|| {
                    ToolFailure::new("view_not_attached", format!("view {view} has no page yet"))
                })?;
                Some((view, target))
            }
        };
        let route = crate::cdp_proxy::tunnel::route(&core.user_settings(), server.port()).await;
        let token = server.token();
        let facts = crate::cdp_proxy::endpoint::Facts {
            port: server.port(),
            token: &token,
            browser_id: server.browser_id(),
        };
        Ok(crate::cdp_proxy::endpoint::answer(
            &facts,
            page.as_ref().map(|(view, target)| (*view, target.as_str())),
            &route,
        ))
    }

    async fn flow_list(&self) -> Result<Value, ToolFailure> {
        let core = self.core().ok_or_else(closed)?;
        crate::flows::app::list(&core)
            .map(|list| masked(&list))
            .map_err(|error| flow_failure(&error))
    }

    async fn flow_run(
        &self,
        name: &str,
        views: Option<Vec<ViewId>>,
        wait: bool,
    ) -> Result<Value, ToolFailure> {
        let core = self.core().ok_or_else(closed)?;
        let started = crate::flows::app::start(&core, name, views, None)
            .map_err(|error| flow_failure(&error))?;
        if !wait {
            let file = crate::flows::result::run_dir(
                &core.host().project_root().unwrap_or_default(),
                &started.run_id,
            )
            .join(crate::flows::result::RESULT_FILE);
            return Ok(json!({ "runId": started.run_id, "resultFile": file.to_string_lossy() }));
        }
        drop(core);
        match started.done.await {
            Ok(Ok(result)) => Ok(masked(&json!(result))),
            Ok(Err(error)) => Err(flow_failure(&error)),
            Err(_) => Err(ToolFailure::new(
                "flows.ended",
                "the run ended without a result (the window closed)",
            )),
        }
    }

    fn latest_findings(&self) -> Option<Value> {
        super::latest::findings()
    }

    fn latest_screenshots(&self) -> Option<Value> {
        let root = self.core().and_then(|core| core.host().project_root());
        super::latest::screenshots(root.as_deref())
    }

    fn latest_screenshot_png(
        &self,
        index: usize,
        max_bytes: usize,
    ) -> Result<Vec<u8>, ToolFailure> {
        let root = self.core().and_then(|core| core.host().project_root());
        super::latest::screenshot_png(root.as_deref(), index, max_bytes)
    }
}

/// A flow's refusal as a tool failure: its code, message and setting.
fn flow_failure(error: &crate::flows::FlowError) -> ToolFailure {
    let failure = ToolFailure::new(error.code(), error.to_string());
    match error.settings_key() {
        Some(key) => failure.with_setting(key),
        None => failure,
    }
}

/// `text` with secrets masked.
fn masked_text(text: &str) -> String {
    rb_diag::redact::redact_text(text)
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
