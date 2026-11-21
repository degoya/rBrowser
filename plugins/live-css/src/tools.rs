//! The tools and commands on the draft, and the page script's requests.
//!
//! - **`set`** (MCP `livecss_set`, the panel's edits): `css` with `breakpoints` (one block) or
//!   `blocks` (the whole draft) replaces the draft and switches it on, unless `enabled` says
//!   otherwise; `enabled` alone switches it on or off; without either it answers the draft.
//! - **`clear`** (MCP `livecss_clear`): the draft without blocks.
//! - **`toggle`** (command): on or off, the blocks kept, said in the status bar.
//! - **`@page`** `{"op": "sheet", "reason": "start" | "refresh"}`: the views' sheet. `start` is a
//!   new document (a navigation, a reload): nothing while `applyOnNavigate` is off.
//!
//! A change is written ([`crate::store`]) and posted: the sheet to every view's page script, and
//! `{"changed": true}` to the panel. Every view gets the same sheet; a scoped block applies by
//! the view's width through its media query ([`crate::media`]).

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::{Value, json};

use crate::draft::{Block, Draft, MAX_CSS_BYTES, block_text};
use crate::media::{Breakpoints, Scope};
use crate::store::{self, Access, SHEET_PATH};
use crate::{bad_args, coded, key};

/// The largest sheet as JSON text: a page script's message is at most 64 KiB.
pub const MAX_SHEET_JSON_BYTES: usize = 60_000;
/// The setting that applies the draft to a new document.
pub const APPLY_ON_NAVIGATE: &str = "applyOnNavigate";

/// What a call works on: the workspace's access, the stored draft and the grid's breakpoints.
struct Context {
    access: Access,
    draft: Draft,
    breakpoints: Breakpoints,
}

fn context(host: &mut dyn PluginHost) -> Context {
    let access = store::access(host);
    let draft = if access == Access::Trusted {
        store::read(host)
    } else {
        Draft::default()
    };
    let breakpoints = Breakpoints::from_views(&host.views().unwrap_or_default());
    Context {
        access,
        draft,
        breakpoints,
    }
}

/// `applyOnNavigate`, on by default; a value that does not read is the default (logged).
fn apply_on_navigate(host: &mut dyn PluginHost) -> bool {
    let Some(text) = host.setting(APPLY_ON_NAVIGATE) else {
        return true;
    };
    serde_json::from_str::<bool>(&text).unwrap_or_else(|_| {
        host.log("warn", "setting applyOnNavigate left out: not usable");
        true
    })
}

/// `args[name]`, none for a missing key or `null`.
fn present<'a>(args: &'a Value, name: &str) -> Option<&'a Value> {
    args.get(name).filter(|value| !value.is_null())
}

/// The draft as the panel and MCP see it.
fn state(context: &Context) -> Value {
    let blocks: Vec<Value> = context
        .draft
        .blocks
        .iter()
        .map(|block| {
            let (scope, media) = match context.breakpoints.scope(&block.breakpoints) {
                Scope::All => ("all", Value::Null),
                Scope::Query(query) => ("query", json!(query)),
                Scope::Nowhere => ("nowhere", Value::Null),
            };
            json!({
                "css": block.css,
                "breakpoints": block.breakpoints,
                "scope": scope,
                "media": media,
                "copy": block_text(block, &context.breakpoints),
            })
        })
        .collect();
    let breakpoints: Vec<Value> = context
        .breakpoints
        .list()
        .iter()
        .map(|bp| json!({ "id": bp.id, "label": bp.label, "width": bp.width }))
        .collect();
    json!({
        "available": context.access.as_str(),
        "enabled": context.draft.enabled,
        "blocks": blocks,
        "breakpoints": breakpoints,
        "file": SHEET_PATH,
    })
}

/// Checks, writes and posts the changed draft; answers it, with `extra` posts after the views'
/// and the panel's.
fn commit(
    host: &mut dyn PluginHost,
    context: &Context,
    extra: Vec<Value>,
) -> Result<Value, Failure> {
    context.draft.check()?;
    let stylesheet = context.draft.stylesheet(&context.breakpoints);
    if json!(stylesheet).to_string().len() > MAX_SHEET_JSON_BYTES {
        return Err(coded(FailureKind::Limit, "errors.too_long").with_param("limit", MAX_CSS_BYTES));
    }
    store::write(host, &context.draft, &stylesheet)?;
    let mut posts = vec![
        json!({ "target": "all", "message": { "css": context.draft.sheet(&context.breakpoints) } }),
        json!({ "target": "panel", "message": { "changed": true } }),
    ];
    posts.extend(extra);
    let mut answer = state(context);
    answer["$post"] = Value::Array(posts);
    Ok(answer)
}

/// `set`.
///
/// # Errors
///
/// Arguments that do not read (`errors.bad_args`), a breakpoint the grid does not show
/// (`errors.unknown_breakpoint`), a draft past its limits, an untrusted workspace or no project,
/// a failed write.
pub fn set(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let enabled = match present(args, "enabled") {
        None => None,
        Some(value) => Some(value.as_bool().ok_or_else(|| bad_args("enabled"))?),
    };
    let blocks = match (present(args, "css"), present(args, "blocks")) {
        (Some(_), Some(_)) => return Err(bad_args("css")),
        (Some(css), None) => {
            let breakpoints = present(args, "breakpoints").cloned().unwrap_or(Value::Null);
            Some(vec![Block::from_json(
                &json!({ "css": css, "breakpoints": breakpoints }),
            )?])
        }
        (None, Some(Value::Array(items))) => Some(
            items
                .iter()
                .map(Block::from_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        (None, Some(_)) => return Err(bad_args("blocks")),
        (None, None) if present(args, "breakpoints").is_some() => {
            return Err(bad_args("breakpoints"));
        }
        (None, None) => None,
    };
    let mut context = context(host);
    if blocks.is_none() && enabled.is_none() {
        return Ok(state(&context));
    }
    if let Some(refusal) = context.access.refusal() {
        return Err(refusal);
    }
    if let Some(blocks) = blocks {
        let unknown = blocks
            .iter()
            .flat_map(|block| &block.breakpoints)
            .find(|id| !context.breakpoints.knows(id));
        if let Some(id) = unknown {
            return Err(
                coded(FailureKind::NotFound, "errors.unknown_breakpoint").with_param("id", id)
            );
        }
        context.draft.blocks = blocks;
        context.draft.enabled = enabled.unwrap_or(true);
    } else if let Some(enabled) = enabled {
        context.draft.enabled = enabled;
    }
    commit(host, &context, Vec::new())
}

/// `clear`: the draft without blocks, on or off as it was.
///
/// # Errors
///
/// An untrusted workspace or no project, a failed write.
pub fn clear(host: &mut dyn PluginHost) -> Result<Value, Failure> {
    let mut context = context(host);
    if let Some(refusal) = context.access.refusal() {
        return Err(refusal);
    }
    context.draft.blocks.clear();
    commit(host, &context, Vec::new())
}

/// `toggle`: the draft on or off, its blocks kept; the status bar says which.
///
/// # Errors
///
/// An untrusted workspace or no project, a failed write.
pub fn toggle(host: &mut dyn PluginHost) -> Result<Value, Failure> {
    let mut context = context(host);
    if let Some(refusal) = context.access.refusal() {
        return Err(refusal);
    }
    context.draft.enabled = !context.draft.enabled;
    let name = if context.draft.enabled {
        "status.on"
    } else {
        "status.off"
    };
    let status = json!({ "target": "status", "message": { "key": key(name), "level": "info", "params": {} } });
    commit(host, &context, vec![status])
}

/// A page script's request: `{"css"}`, the sheet of the view - none outside a trusted workspace.
///
/// # Errors
///
/// `errors.bad_args` for a request that is not `sheet`.
pub fn page(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let message = args.get("message").unwrap_or(&Value::Null);
    if message.get("op").and_then(Value::as_str) != Some("sheet") {
        return Err(bad_args("op"));
    }
    let start = match message.get("reason").and_then(Value::as_str) {
        Some("start") => true,
        Some("refresh") => false,
        _ => return Err(bad_args("reason")),
    };
    if start && !apply_on_navigate(host) {
        return Ok(json!({ "css": "" }));
    }
    let context = context(host);
    if context.access != Access::Trusted {
        return Ok(json!({ "css": "" }));
    }
    Ok(json!({ "css": context.draft.sheet(&context.breakpoints) }))
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
