//! The `target` of every view tool: `"all"`, a view id, or a breakpoint id that names the
//! views showing it (`"mobile"`). Resolved against the views of the moment; an unknown target
//! is an error that lists the views there are.
//!
//! When `target` is absent (RB-099-16): a tool that reads, navigates or runs over the views takes
//! `"all"`; one that acts in a page as a person would takes the active view
//! (`coverage::tables::ACTIVE_DEFAULT`); a few say a default of their own (`OWN_DEFAULT`); a
//! one-view tool takes the active view. Each tool's `target` text says which, and the coverage
//! test holds the text to the tables (`coverage::tools::target_text_problem`).

use rb_engine::{Target, ViewId, ViewInfo};
use rmcp::schemars;
use serde::{Deserialize, Serialize};

use crate::backend::ToolFailure;

/// The code of an unknown target.
pub const VIEW_NOT_FOUND: &str = "view.not_found";

/// `"all"`, a view id or a breakpoint id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum TargetArg {
    /// A view id.
    Id(u32),
    /// `"all"`, a view id as text or a breakpoint id.
    Name(String),
}

impl TargetArg {
    /// `"all"`.
    #[must_use]
    pub fn all() -> Self {
        Self::Name("all".to_owned())
    }
}

/// A resolved target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// Every view.
    All,
    /// These views, in grid order.
    Views(Vec<ViewId>),
}

impl Resolved {
    /// The targets one command call each: `all` once, else one per view.
    #[must_use]
    pub fn targets(&self) -> Vec<Target> {
        match self {
            Resolved::All => vec![Target::All],
            Resolved::Views(views) => views.iter().copied().map(Target::View).collect(),
        }
    }

    /// The views it covers among `views`.
    #[must_use]
    pub fn view_ids(&self, views: &[ViewInfo]) -> Vec<ViewId> {
        match self {
            Resolved::All => views.iter().map(|view| view.id).collect(),
            Resolved::Views(ids) => ids.clone(),
        }
    }
}

/// Resolves `arg` (absent: every view) against `views`.
///
/// # Errors
///
/// No view matches: [`VIEW_NOT_FOUND`] with the known views.
pub fn resolve(arg: Option<&TargetArg>, views: &[ViewInfo]) -> Result<Resolved, ToolFailure> {
    let Some(arg) = arg else {
        return Ok(Resolved::All);
    };
    let (shown, id) = match arg {
        TargetArg::Id(id) => (id.to_string(), Some(*id)),
        TargetArg::Name(name) if name.eq_ignore_ascii_case("all") => return Ok(Resolved::All),
        TargetArg::Name(name) => (name.clone(), name.trim().parse::<u32>().ok()),
    };
    let found: Vec<ViewId> = match id {
        Some(id) => views
            .iter()
            .filter(|view| view.id == ViewId(id))
            .map(|view| view.id)
            .collect(),
        None => views
            .iter()
            .filter(|view| {
                view.breakpoint.as_deref() == Some(shown.as_str())
                    || view.cell_breakpoint.as_deref() == Some(shown.as_str())
            })
            .map(|view| view.id)
            .collect(),
    };
    if found.is_empty() {
        return Err(not_found(&shown, views));
    }
    Ok(Resolved::Views(found))
}

/// Resolves `arg` to exactly one view (DevTools inspect one view); absent: `None`, the active
/// view.
///
/// # Errors
///
/// No view matches, or `all` or a breakpoint shown in several views.
pub fn resolve_one(
    arg: Option<&TargetArg>,
    views: &[ViewInfo],
) -> Result<Option<ViewId>, ToolFailure> {
    match resolve(arg, views)? {
        Resolved::Views(ids) if ids.len() == 1 => Ok(ids.first().copied()),
        Resolved::All if arg.is_none() => Ok(None),
        _ => Err(ToolFailure::new(
            "target.not_single",
            format!(
                "this tool acts on one view; pass a view id. Known views: {}",
                known(views)
            ),
        )),
    }
}

/// The failure of a target no view matches, listing the known views.
pub(crate) fn not_found(shown: &str, views: &[ViewInfo]) -> ToolFailure {
    ToolFailure::new(
        VIEW_NOT_FOUND,
        format!("view `{shown}` not found; known views: {}", known(views)),
    )
}

/// The views as `1 (mobile), 2 (desktop)`, or `none`.
fn known(views: &[ViewInfo]) -> String {
    if views.is_empty() {
        return "none".to_owned();
    }
    views
        .iter()
        .map(|view| match &view.breakpoint {
            Some(breakpoint) => format!("{} ({breakpoint})", view.id),
            None => format!("{} ({})", view.id, view.label),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
