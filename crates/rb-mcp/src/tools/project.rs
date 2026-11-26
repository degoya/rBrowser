//! Project, config and settings: `project_info`, `project_open`, `config_get`, `settings_get`,
//! `settings_set` (RB-030-03, the structure in [`crate::settings_view`]). Reads are masked by the
//! host; `settings_set` writes as an MCP caller, so an `x-protected` key (MCP itself, trust,
//! plugins, the sandbox) is refused - a trust decision and a plugin's trust are a person's - and
//! the host logs and shows every change.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use super::NoArgs;
use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::{SettingScope, ToolFailure};
use crate::settings_view::{self, Filter};

/// `config_get`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyParams {
    /// A dotted key (`layout.gap`); everything when absent.
    #[serde(default)]
    pub key: Option<String>,
}

/// `settings_get`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsParams {
    /// One dotted key (`sync.scroll`, `shortcuts.bindings`); the whole structure when absent.
    #[serde(default)]
    pub key: Option<String>,
    /// A category id (`general`, `sync`, `automation`, `shortcuts`, ...).
    #[serde(default)]
    pub category: Option<String>,
    /// A section id (`mcp`), inside `category` when both are given.
    #[serde(default)]
    pub section: Option<String>,
    /// A search as in the settings window: every word in a key's label, key or description.
    #[serde(default)]
    pub query: Option<String>,
}

/// `settings_set`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetParams {
    /// The dotted key.
    pub key: String,
    /// The new value; `null` resets the key to its default.
    pub value: Value,
    /// `user` (default), `project` (`.rbrowser/config.json`) or `local`
    /// (`.rbrowser/config.local.json`).
    #[serde(default)]
    pub scope: SettingScope,
}

/// `project_open`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenParams {
    /// A recent project's folder (`settings_get` key `recentProjects`).
    #[serde(default)]
    pub path: Option<String>,
    /// The index in the recent projects.
    #[serde(default)]
    pub index: Option<u32>,
}

/// The code of a key that is not there.
pub const KEY_NOT_FOUND: &str = "settings.key_not_found";

#[tool_router(router = project_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The open project: root, config file and whether the workspace is trusted (read only; trust is decided by a person in rBrowser, never over MCP)."
    )]
    pub(crate) async fn project_info(&self, _args: Parameters<NoArgs>) -> ToolResult {
        respond(self.read(|backend| backend.project_info()).await)
    }

    #[tool(
        description = "Open a recent project in rBrowser (its own window): path names one of the recent projects, index its place in the list; without either the most recent other project."
    )]
    pub(crate) async fn project_open(
        &self,
        Parameters(params): Parameters<OpenParams>,
    ) -> ToolResult {
        let args = object([
            ("path", params.path.map(Value::String)),
            ("index", params.index.map(Value::from)),
        ]);
        respond(self.command("project.open_recent", args).await)
    }

    #[tool(
        description = "Read the effective project config (.rbrowser/config.json merged with config.local.json), secrets masked; key picks one dotted key (breakpoints, layout.gap)."
    )]
    pub(crate) async fn config_get(&self, Parameters(params): Parameters<KeyParams>) -> ToolResult {
        let config = self.read(|backend| backend.config()).await;
        respond(config.and_then(|config| pick(config, params.key.as_deref())))
    }

    #[tool(
        description = "Read the settings in the settings window's structure: categories, sections and keys in order, each key with type, value, origin, default, label, description, scopes, whether MCP may write it and its deep link rbrowser://settings/<key>; secrets masked. key picks one; category, section and query (every word in label, key or description) narrow it."
    )]
    pub(crate) async fn settings_get(
        &self,
        Parameters(params): Parameters<SettingsParams>,
    ) -> ToolResult {
        let settings = self.read(|backend| backend.settings()).await;
        respond(settings.and_then(|settings| {
            settings_view::view(
                &settings,
                &Filter {
                    key: params.key.as_deref(),
                    category: params.category.as_deref(),
                    section: params.section.as_deref(),
                    query: params.query.as_deref(),
                },
            )
        }))
    }

    #[tool(
        description = "Change a setting as a person would in Settings: key, value (null resets), scope user (default), project or local; answers the new value, its origin and the deep link. Shortcuts: key shortcuts.bindings.<commandId> with a list of bindings (null restores the default), refused when reserved or taken. Protected keys (MCP, trust, plugins, the sandbox, program paths, network, headers, breakpoints - use the breakpoint tools -, extensions and others) are refused with code settings.rejected.protected. Every change is logged and shown in rBrowser."
    )]
    pub(crate) async fn settings_set(
        &self,
        Parameters(params): Parameters<SetParams>,
    ) -> ToolResult {
        let SetParams { key, value, scope } = params;
        let written = self
            .read(move |backend| -> Result<Value, ToolFailure> {
                backend.set_setting(&key, &value, scope)?;
                let fallback = json!({ "key": key, "deepLink": settings_view::deep_link(&key) });
                let mut answer =
                    settings_view::entry(&backend.settings(), &key).unwrap_or(fallback);
                if let Some(object) = answer.as_object_mut() {
                    object.insert("scope".to_owned(), json!(scope));
                }
                Ok(answer)
            })
            .await;
        respond(written.and_then(|result| result))
    }
}

/// The value at the dotted `key` of `value`, or all of it.
fn pick(value: Value, key: Option<&str>) -> Result<Value, ToolFailure> {
    let Some(key) = key else {
        return Ok(value);
    };
    key.split('.')
        .try_fold(&value, |current, part| current.get(part))
        .cloned()
        .ok_or_else(|| ToolFailure::new(KEY_NOT_FOUND, format!("the config has no key `{key}`")))
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
