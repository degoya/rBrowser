//! The plugins' MCP tools (RB-060-12, `docs/plugins.md`, "MCP"): every `[[tools]]` entry of a
//! loaded feature plugin as `<mcp_prefix>_<tool>`, listed beside the core tools. A plugin tool
//! takes `target` like every view tool - `"all"`, a view id or a breakpoint id, resolved here
//! and handed to the plugin as `"all"` or a view id, one call per view - and runs through the
//! host (`Backend::host_tool`), which calls the plugin's `feature.call-tool`. A plugin tool never
//! shadows a core tool.

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
use crate::target::TargetArg;

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
        properties.insert(
            "target".to_owned(),
            json!({
                "description": TARGET_DESCRIPTION,
                "anyOf": [{ "type": "string" }, { "type": "integer", "minimum": 0 }],
            }),
        );
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
        let result = match target_of(args.remove("target")) {
            Ok(target) => {
                context
                    .service
                    .host_per_target(&name, target.as_ref(), Value::Object(args), Answers::Joined)
                    .await
            }
            Err(failure) => Err(failure),
        };
        respond(result).map(Into::into)
    })
}

/// The `target` argument, when there is one.
fn target_of(value: Option<Value>) -> Result<Option<TargetArg>, ToolFailure> {
    value
        .map(|value| {
            serde_json::from_value(value).map_err(|_| {
                ToolFailure::new(
                    "invalid_arguments",
                    "target is \"all\", a view id or a breakpoint id",
                )
            })
        })
        .transpose()
}

#[cfg(test)]
#[path = "plugins_tests.rs"]
mod tests;
