//! Writing the project config: `config_set` and `breakpoint_add`, `breakpoint_update`,
//! `breakpoint_remove`. Each reads the file, edits it ([`crate::config_edit`]: keys guarded by
//! workspace trust refused) and hands it to the host, which checks the result as the settings
//! window does (`rb_config::check`) before it writes; the watcher then reloads the grid.
//! `breakpoint_remove` edits both project files: the breakpoint leaves each file that defines
//! it (RB-094-25), its references go too.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::{ConfigScope, ToolFailure};
use crate::config_edit;

/// `config_set`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigSetParams {
    /// The dotted key (`layout.gap`, `startUrl`, `emulation.locale`).
    pub key: String,
    /// The new value; `null` removes the key.
    pub value: Value,
    /// `project` (default, `.rbrowser/config.json`) or `local` (`config.local.json`).
    #[serde(default)]
    pub scope: ConfigScope,
}

/// `breakpoint_add`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BreakpointAddParams {
    /// The breakpoint as in `breakpoints[]`: `id`, `width`, `height`, `label`, `device`,
    /// `emulation`, ... (no `headers`).
    pub breakpoint: Map<String, Value>,
    /// `project` (default) or `local`.
    #[serde(default)]
    pub scope: ConfigScope,
}

/// `breakpoint_update`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BreakpointUpdateParams {
    /// The breakpoint's id.
    pub id: String,
    /// The fields to change; objects merge, `null` removes a field.
    pub fields: Map<String, Value>,
    /// `project` (default) or `local`.
    #[serde(default)]
    pub scope: ConfigScope,
}

/// `breakpoint_remove`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BreakpointRemoveParams {
    /// The breakpoint's id.
    pub id: String,
    /// `project` (default) or `local`: the file read first; the breakpoint leaves both files.
    #[serde(default)]
    pub scope: ConfigScope,
}

#[tool_router(router = config_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Change one key of the project config (.rbrowser/config.json, or config.local.json with scope local): key dotted, value (null removes). The result is checked as the settings window checks it. Keys guarded by workspace trust (headers, network, plugins, extensions), protected keys (of the core and of every installed plugin) and a write at a parent key that would remove or change one are refused with code config_key_protected."
    )]
    pub(crate) async fn config_set(
        &self,
        Parameters(params): Parameters<ConfigSetParams>,
    ) -> ToolResult {
        let ConfigSetParams { key, value, scope } = params;
        let answer = json!({ "key": key, "scope": scope });
        let result = self
            .read(move |backend| {
                // The protected keys of every installed plugin, loaded or not (RB-094-31): never
                // written, removed or changed, also not through a parent key.
                let protected = backend.mcp_protected_keys();
                // Refused before the file is read, so a protected key is answered as one also
                // with no project open; `set` checks again with the parent-change rule.
                if let Some(refused) = config_edit::write_refusal(&key, &value)
                    .or_else(|| config_edit::protected_refusal(&key, &value, &protected))
                {
                    return Err(refused);
                }
                let mut document = backend.config_document(scope)?;
                config_edit::set(&mut document, &key, value, &protected)?;
                backend.write_config_document(scope, &document)?;
                Ok::<_, ToolFailure>(answer)
            })
            .await;
        respond(result.and_then(|result| result))
    }

    #[tool(
        description = "Add a breakpoint to the project config: breakpoint as in breakpoints[] (id, width, height, label, device, emulation, ...; request headers are refused). Checked before it is written; the grid follows."
    )]
    pub(crate) async fn breakpoint_add(
        &self,
        Parameters(params): Parameters<BreakpointAddParams>,
    ) -> ToolResult {
        let id = params.breakpoint.get("id").cloned().unwrap_or(Value::Null);
        let answer = json!({ "added": id });
        self.edit_config(params.scope, answer, move |document| {
            config_edit::add_breakpoint(document, params.breakpoint)
        })
        .await
    }

    #[tool(
        description = "Change a breakpoint of the project config: id, fields to merge (null removes a field; request headers are refused). Checked before it is written."
    )]
    pub(crate) async fn breakpoint_update(
        &self,
        Parameters(params): Parameters<BreakpointUpdateParams>,
    ) -> ToolResult {
        let answer = json!({ "updated": params.id });
        self.edit_config(params.scope, answer, move |document| {
            config_edit::update_breakpoint(document, &params.id, params.fields)
        })
        .await
    }

    #[tool(
        description = "Remove a breakpoint from the project config by id: from both project files where either defines it, with every reference to it in both (layout cells, hidden views, split and canvas fields, a preset's focus, envCompare.breakpoints). Checked before it is written; the answer counts the references removed."
    )]
    pub(crate) async fn breakpoint_remove(
        &self,
        Parameters(params): Parameters<BreakpointRemoveParams>,
    ) -> ToolResult {
        let BreakpointRemoveParams { id, scope } = params;
        let other_scope = match scope {
            ConfigScope::Project => ConfigScope::Local,
            ConfigScope::Local => ConfigScope::Project,
        };
        let result = self
            .read(move |backend| {
                let mut document = backend.config_document(scope)?;
                let mut other = backend.config_document(other_scope)?;
                let untouched = other.clone();
                let defined_here = config_edit::defines_breakpoint(&document, &id);
                let references = config_edit::remove_breakpoint_everywhere(
                    &mut document,
                    &mut other,
                    scope,
                    &id,
                )?;
                // First the file that is valid beside the other one on disk, which still defines
                // the breakpoint (RB-090-61): the other file, unless only it defined the
                // breakpoint - then this one (RB-094-25).
                let other_changed = other != untouched;
                if defined_here && other_changed {
                    backend.write_config_document(other_scope, &other)?;
                }
                backend.write_config_document(scope, &document)?;
                if !defined_here && other_changed {
                    backend.write_config_document(other_scope, &other)?;
                }
                Ok::<_, ToolFailure>(json!({ "removed": id, "references": references }))
            })
            .await;
        respond(result.and_then(|result| result))
    }
}

impl RbMcpServer {
    /// Reads the file `scope`, applies `edit`, has the host check and write it; answers
    /// `answer`.
    async fn edit_config(
        &self,
        scope: ConfigScope,
        answer: Value,
        edit: impl FnOnce(&mut Value) -> Result<(), ToolFailure> + Send + 'static,
    ) -> ToolResult {
        let result = self
            .read(move |backend| {
                let mut document = backend.config_document(scope)?;
                edit(&mut document)?;
                backend.write_config_document(scope, &document)?;
                Ok::<_, ToolFailure>(answer)
            })
            .await;
        respond(result.and_then(|result| result))
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
