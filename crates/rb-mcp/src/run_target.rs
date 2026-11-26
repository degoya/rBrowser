//! The `target` of the test-run tools (RB-099-10): `checks_run`, `visual_compare`,
//! `visual_baseline_update`, `flow_run` and the plugin tools that take `breakpoints`
//! (`lighthouse_run`). Beside what every view tool takes - `"all"`, a view id or a breakpoint id
//! ([`TargetArg`], unchanged: [`Split::Target`]) - it takes `"visible"`, the views the grid
//! shows, and a list of view ids and breakpoint ids ([`Split::Scope`]): the app resolves the
//! scope once when the run starts, in its one resolution (`rb_config::resolve_scope`). A view id
//! in a list stands for its breakpoint. The other view tools keep [`TargetArg`] as it is.

use rb_config::ViewScope;
use rb_engine::ViewInfo;
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::backend::ToolFailure;
use crate::target::{TargetArg, not_found};

/// The code of an empty list.
pub const EMPTY_SELECTION: &str = "target.empty_selection";
/// The code of a listed view that shows no breakpoint (a free size).
pub const NO_BREAKPOINT: &str = "target.no_breakpoint";
/// The word of the views the grid shows.
pub const VISIBLE: &str = "visible";

/// One entry of a target list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum TargetItem {
    /// A view id (`views_list`).
    Id(u32),
    /// A view id as text, or a breakpoint id.
    Name(String),
}

/// What a test-run tool's `target` says, as written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum RunTarget {
    /// A view id.
    Id(u32),
    /// `"all"`, `"visible"`, a view id as text, or a breakpoint id.
    Name(String),
    /// View ids and breakpoint ids: the views of those breakpoints.
    List(Vec<TargetItem>),
}

/// A [`RunTarget`] taken apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Split {
    /// A target every view tool takes: one call per view as before.
    Target(Option<TargetArg>),
    /// A scope the app resolves: one call with `scope`.
    Scope(ViewScope),
}

/// Takes `target` apart against `views` (a list's view ids stand for their breakpoints).
///
/// # Errors
///
/// An empty list ([`EMPTY_SELECTION`]), a listed view id no view has ([`crate::target`]'s
/// `view.not_found`, with the known views) or one that shows no breakpoint ([`NO_BREAKPOINT`]).
pub fn split(target: Option<&RunTarget>, views: &[ViewInfo]) -> Result<Split, ToolFailure> {
    let items = match target {
        None => return Ok(Split::Target(None)),
        Some(RunTarget::Id(id)) => return Ok(Split::Target(Some(TargetArg::Id(*id)))),
        Some(RunTarget::Name(name)) if name.trim().eq_ignore_ascii_case(VISIBLE) => {
            return Ok(Split::Scope(ViewScope::Visible));
        }
        Some(RunTarget::Name(name)) => {
            return Ok(Split::Target(Some(TargetArg::Name(name.clone()))));
        }
        Some(RunTarget::List(items)) => items,
    };
    if items.is_empty() {
        return Err(ToolFailure::new(
            EMPTY_SELECTION,
            "the target list is empty; name at least one view id or breakpoint id",
        ));
    }
    let mut breakpoints: Vec<String> = Vec::new();
    for item in items {
        let breakpoint = match item {
            TargetItem::Id(id) => breakpoint_of(*id, views)?,
            TargetItem::Name(name) => match name.trim().parse::<u32>() {
                Ok(id) => breakpoint_of(id, views)?,
                Err(_) => name.trim().to_owned(),
            },
        };
        if !breakpoints.contains(&breakpoint) {
            breakpoints.push(breakpoint);
        }
    }
    Ok(Split::Scope(ViewScope::Breakpoints(breakpoints)))
}

/// The breakpoint view `id` shows (or its cell's).
fn breakpoint_of(id: u32, views: &[ViewInfo]) -> Result<String, ToolFailure> {
    let view = views
        .iter()
        .find(|view| view.id.0 == id)
        .ok_or_else(|| not_found(&id.to_string(), views))?;
    view.breakpoint
        .clone()
        .or_else(|| view.cell_breakpoint.clone())
        .ok_or_else(|| {
            ToolFailure::new(
                NO_BREAKPOINT,
                format!("view {id} shows a free size, no breakpoint; pass it alone as the target"),
            )
        })
}

impl crate::RbMcpServer {
    /// [`split`] against the views now; they are read for a list only.
    pub(crate) async fn split_target(
        &self,
        target: Option<&RunTarget>,
    ) -> Result<Split, ToolFailure> {
        let views = match target {
            Some(RunTarget::List(_)) => self.read(|backend| backend.views()).await?,
            _ => Vec::new(),
        };
        split(target, &views)
    }
}

/// `args` (an object; anything else counts as none) with `scope`.
#[must_use]
pub fn with_scope(args: &Value, scope: &ViewScope) -> Value {
    let mut call = match args {
        Value::Object(fields) => Value::Object(fields.clone()),
        _ => json!({}),
    };
    call["scope"] = json!(scope);
    call
}

#[cfg(test)]
#[path = "run_target_tests.rs"]
mod tests;
