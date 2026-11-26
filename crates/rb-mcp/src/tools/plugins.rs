//! The plugins' MCP tools (RB-060-12, `docs/plugins.md`, "MCP"): every `[[tools]]` entry of a
//! loaded feature plugin as `<mcp_prefix>_<tool>`, listed beside the core tools. A plugin tool
//! takes `target` like every view tool - `"all"`, a view id or a breakpoint id, resolved here
//! and handed to the plugin as `"all"` or a view id, one call per view - and runs through the
//! host (`Backend::host_tool`), which calls the plugin's `feature.call-tool`. A plugin tool never
//! shadows a core tool. A tool that takes `breakpoints` (`lighthouse_run`) takes `target`
//! `"visible"` and a list of view and breakpoint ids too (RB-099-10): one call with the scope as
//! `$scope`, which the host turns into the breakpoints.

use std::future::Future;
use std::pin::Pin;

use rmcp::handler::server::router::tool::{ToolRoute, ToolRouter};
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResponse, Tool};
use serde_json::{Map, Value, json};

use super::forward::Answers;
use crate::RbMcpServer;
use crate::answer::respond;
use crate::backend::ToolFailure;
use crate::run_target::{RunTarget, Split};

/// One plugin tool as the host lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginToolInfo {
    /// `<mcp_prefix>_<tool>`.
    pub name: String,
    /// What the tool does, in English (the plugin's locales).
    pub description: String,
    /// The JSON schema of its arguments, from the manifest.
    pub input_schema: Value,
}

/// The description of `target` every plugin tool's schema gets.
const TARGET_DESCRIPTION: &str =
    "\"all\" (default), a view id or a breakpoint id; the plugin gets \"all\" or a view id";

/// The description of `target` of a plugin tool that takes `breakpoints` (RB-099-10).
const SCOPED_TARGET_DESCRIPTION: &str = "\"all\" (default), \"visible\" (the views the grid \
     shows), a view id, a breakpoint id, or a list of view ids and breakpoint ids (their \
     breakpoints; an empty list or an unknown id is an error)";

/// The argument a plugin tool takes chosen breakpoints in.
const BREAKPOINTS: &str = "breakpoints";

/// Whether `schema` (a tool's arguments) takes `breakpoints`.
fn takes_breakpoints(schema: &Value) -> bool {
    schema
        .get("properties")
        .and_then(|properties| properties.get(BREAKPOINTS))
        .is_some()
}

/// `schema` (an object schema; anything else counts as `{ "type": "object" }`) with the
/// property `target`.
#[must_use]
pub fn with_target(schema: &Value) -> Map<String, Value> {
    let mut schema = match schema {
        Value::Object(fields) if fields.get("type") == Some(&json!("object")) => fields.clone(),
        _ => Map::from_iter([("type".to_owned(), json!("object"))]),
    };
    let properties = schema
        .entry("properties")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(properties) = properties {
        let target = if properties.contains_key(BREAKPOINTS) {
            json!({
                "description": SCOPED_TARGET_DESCRIPTION,
                "anyOf": [
                    { "type": "string" },
                    { "type": "integer", "minimum": 0 },
                    {
                        "type": "array",
                        "items": { "anyOf": [{ "type": "string" }, { "type": "integer", "minimum": 0 }] }
                    },
                ],
            })
        } else {
            json!({
                "description": TARGET_DESCRIPTION,
                "anyOf": [{ "type": "string" }, { "type": "integer", "minimum": 0 }],
            })
        };
        properties.insert("target".to_owned(), target);
    }
    schema
}

/// The routes of `tools`; a name a core tool has is left out.
pub(crate) fn router(tools: Vec<PluginToolInfo>) -> ToolRouter<RbMcpServer> {
    let core = RbMcpServer::router();
    let mut router = ToolRouter::new();
    for info in tools {
        if core.has_route(&info.name) {
            tracing::warn!(
                tool = info.name.as_str(),
                "a plugin tool shadows a core tool; left out"
            );
            continue;
        }
        let tool = Tool::new(info.name, info.description, with_target(&info.input_schema));
        router.add_route(ToolRoute::new_dyn(tool, call));
    }
    router
}

type Answer<'a> =
    Pin<Box<dyn Future<Output = Result<CallToolResponse, rmcp::ErrorData>> + Send + 'a>>;

fn call(context: ToolCallContext<'_, RbMcpServer>) -> Answer<'_> {
    Box::pin(async move {
        let name = context.name.to_string();
        let mut args = context.arguments.unwrap_or_default();
        let service = context.service;
        let result = async {
            let target = target_of(args.remove("target"))?;
            match service.split_target(target.as_ref()).await? {
                Split::Target(target) => {
                    service
                        .host_per_target(
                            &name,
                            target.as_ref(),
                            Value::Object(args),
                            Answers::Joined,
                        )
                        .await
                }
                Split::Scope(scope) => {
                    let tools = service.read(|backend| backend.plugin_tools()).await?;
                    if !tools
                        .iter()
                        .any(|tool| tool.name == name && takes_breakpoints(&tool.input_schema))
                    {
                        return Err(ToolFailure::new(
                            "args.invalid",
                            "this tool takes target \"all\", a view id or a breakpoint id",
                        ));
                    }
                    // RB-099-10: one call; the host resolves `$scope` into `breakpoints`.
                    args.insert("$scope".to_owned(), json!(scope));
                    service.backend.host_tool(&name, Value::Object(args)).await
                }
            }
        }
        .await;
        respond(result).map(Into::into)
    })
}

/// The `target` argument, when there is one.
fn target_of(value: Option<Value>) -> Result<Option<RunTarget>, ToolFailure> {
    value
        .map(|value| {
            serde_json::from_value(value).map_err(|_| {
                ToolFailure::new(
                    "args.invalid",
                    "target is \"all\", a view id, a breakpoint id, \"visible\" or a list of \
                     view ids and breakpoint ids",
                )
            })
        })
        .transpose()
}

#[cfg(test)]
#[path = "plugins_tests.rs"]
mod tests;
