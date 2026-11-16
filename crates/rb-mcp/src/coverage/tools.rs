//! The registered tools themselves: one name each, a description, an object schema for the
//! arguments, and `target: "all" | viewId` on every tool that acts on views.

use std::collections::BTreeSet;

use rmcp::model::Tool;
use serde_json::Value;

use super::tables::{self, SINGLE_VIEW};
use crate::RbMcpServer;

/// The shortest description a tool may have.
const MIN_DESCRIPTION: usize = 20;
/// How deep a `$ref` chain is followed.
const MAX_DEPTH: usize = 16;

/// What a tool's `target` takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetSupport {
    /// `"all"` (the default), a view id or a breakpoint id.
    All,
    /// Exactly one view ([`SINGLE_VIEW`]); the active view when absent.
    One,
    /// No `target`: the tool acts on no view ([`tables::UNTARGETED`]).
    None,
}

impl TargetSupport {
    /// The word of `schema/mcp-tools.v1.json`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            TargetSupport::All => "all",
            TargetSupport::One => "one",
            TargetSupport::None => "none",
        }
    }
}

/// Every tool area by area, as the routers hold them before [`RbMcpServer::router`] merges
/// them (a name two areas share would vanish in the merge), each sorted by name. A new router
/// joins here as it joins `router()`; [`tool_problems`] holds the two together.
#[must_use]
pub fn areas() -> Vec<(&'static str, Vec<Tool>)> {
    let mut areas = vec![
        ("views", RbMcpServer::views_router().list_all()),
        ("views", RbMcpServer::views_edit_router().list_all()),
        ("project", RbMcpServer::project_router().list_all()),
        ("layout", RbMcpServer::layout_router().list_all()),
        ("app", RbMcpServer::app_router().list_all()),
        ("page", RbMcpServer::page_router().list_all()),
        ("logs", RbMcpServer::logs_router().list_all()),
        ("config", RbMcpServer::config_router().list_all()),
        ("sync", RbMcpServer::sync_router().list_all()),
        ("capture", RbMcpServer::capture_router().list_all()),
        ("cdp", RbMcpServer::cdp_router().list_all()),
        ("flows", RbMcpServer::flows_router().list_all()),
        ("engines", RbMcpServer::engines_router().list_all()),
        ("cookies", RbMcpServer::cookies_router().list_all()),
        ("recorder", RbMcpServer::recorder_router().list_all()),
        ("visual", RbMcpServer::visual_router().list_all()),
        ("sitemap", RbMcpServer::sitemap_router().list_all()),
        ("env_compare", RbMcpServer::env_compare_router().list_all()),
        ("metrics", RbMcpServer::metrics_router().list_all()),
        ("video", RbMcpServer::video_router().list_all()),
        ("status", RbMcpServer::status_router().list_all()),
        ("extensions", RbMcpServer::extensions_router().list_all()),
        (
            "network_debug",
            RbMcpServer::network_debug_router().list_all(),
        ),
        ("portal", RbMcpServer::portal_router().list_all()),
        ("devtools", RbMcpServer::devtools_router().list_all()),
        ("bookmarks", RbMcpServer::bookmarks_router().list_all()),
        ("panels", RbMcpServer::panels_router().list_all()),
        ("views", RbMcpServer::page_zoom_router().list_all()),
        ("views", RbMcpServer::force_state_router().list_all()),
    ];
    for (_, tools) in &mut areas {
        tools.sort_by(|a, b| a.name.cmp(&b.name));
    }
    areas
}

/// Every problem of the registered tools, each as `<tool>: <why>`.
#[must_use]
pub fn tool_problems() -> Vec<String> {
    let areas = areas();
    let mut problems = Vec::new();
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for (area, tools) in &areas {
        for tool in tools {
            let name = tool.name.as_ref();
            if let Some((_, first)) = seen.iter().find(|(known, _)| *known == name) {
                problems.push(format!(
                    "{name}: registered by the areas {first} and {area}; tool names are unique"
                ));
            } else {
                seen.push((name, area));
            }
            problems.extend(
                definition_problems(tool)
                    .into_iter()
                    .map(|why| format!("{name}: {why}")),
            );
        }
    }
    let listed: BTreeSet<&str> = seen.iter().map(|(name, _)| *name).collect();
    let served = RbMcpServer::tool_names();
    for name in &served {
        if !listed.contains(name.as_str()) {
            problems.push(format!(
                "{name}: served by router() but in no area of coverage::tools::areas()"
            ));
        }
    }
    for (table, tool) in tables::named() {
        if !served.iter().any(|name| name == tool) {
            problems.push(format!(
                "{tool}: {table} names it, but no such tool is registered"
            ));
        }
    }
    problems
}

/// What is wrong with one tool's definition: its description, its schema, its `target`.
#[must_use]
pub fn definition_problems(tool: &Tool) -> Vec<String> {
    let mut problems = Vec::new();
    let described = tool
        .description
        .as_deref()
        .is_some_and(|text| text.trim().len() >= MIN_DESCRIPTION);
    if !described {
        problems.push(format!(
            "no description of at least {MIN_DESCRIPTION} characters"
        ));
    }
    if tool.input_schema.get("type") != Some(&Value::from("object")) {
        problems.push("the arguments' schema is not a JSON object schema".to_owned());
    }
    if let Err(why) = target_support(tool) {
        problems.push(why);
    }
    problems
}

/// What `tool`'s `target` takes, held to the tables: a tool without `target` is listed in
/// [`tables::UNTARGETED`]; a `target` takes a view id (integer) and text (`"all"`, a breakpoint
/// id) and says `"all"`, unless the tool is in [`SINGLE_VIEW`].
///
/// # Errors
///
/// The rule it breaks, in English.
pub fn target_support(tool: &Tool) -> Result<TargetSupport, String> {
    let name = tool.name.as_ref();
    let root = Value::Object((*tool.input_schema).clone());
    let untargeted = tables::untargeted(name).is_some();
    let single = SINGLE_VIEW.contains(&name);
    let Some(target) = root.get("properties").and_then(|props| props.get("target")) else {
        if untargeted {
            return Ok(TargetSupport::None);
        }
        return Err(
            "takes no target: a tool that acts on views takes target (\"all\" | viewId); one \
             that does not is listed in UNTARGETED"
                .to_owned(),
        );
    };
    if untargeted {
        return Err("takes target but is listed in UNTARGETED".to_owned());
    }
    let types = accepted_types(target, &root, 0);
    if !(types.contains("integer") && types.contains("string")) {
        return Err(format!(
            "its target accepts {types:?}; it takes a view id (integer) and \"all\" or a \
             breakpoint id (string): TargetArg"
        ));
    }
    if single {
        return Ok(TargetSupport::One);
    }
    let says_all = target
        .get("description")
        .and_then(Value::as_str)
        .is_some_and(|text| text.contains("\"all\""));
    if says_all {
        Ok(TargetSupport::All)
    } else {
        Err(
            "its target does not say \"all\": a view tool takes \"all\" | viewId, or is listed \
             in SINGLE_VIEW"
                .to_owned(),
        )
    }
}

/// The JSON types `schema` accepts, `$ref`s into `root` followed, `anyOf`/`oneOf`/`allOf`
/// joined.
fn accepted_types(schema: &Value, root: &Value, depth: usize) -> BTreeSet<String> {
    let mut types = BTreeSet::new();
    if depth > MAX_DEPTH {
        return types;
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str)
        && let Some(target) = resolve(reference, root)
    {
        types.extend(accepted_types(target, root, depth + 1));
    }
    match schema.get("type") {
        Some(Value::String(kind)) => {
            types.insert(kind.clone());
        }
        Some(Value::Array(kinds)) => {
            types.extend(kinds.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        _ => {}
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(Value::Array(parts)) = schema.get(key) {
            for part in parts {
                types.extend(accepted_types(part, root, depth + 1));
            }
        }
    }
    types
}

/// `#/$defs/<name>` or `#/definitions/<name>` (a JSON pointer) in `root`.
fn resolve<'a>(reference: &str, root: &'a Value) -> Option<&'a Value> {
    root.pointer(reference.strip_prefix('#')?)
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
