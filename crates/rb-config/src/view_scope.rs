//! Which breakpoints a run takes (RB-099-08, shared with RB-099-09 and RB-099-10): every one of
//! the config, the ones the grid shows, or those named.
//!
//! - `"all"`: every breakpoint, in config order.
//! - `"visible"`: the breakpoints the grid shows - the selection in effect ([`hidden_selection`],
//!   which a chosen preset's `hidden` has become) left out and, in a grid with cells, only the
//!   breakpoints in a cell.
//! - `["id", ...]`: the breakpoints named; an unknown id is refused.
//!
//! The result keeps config order and names each breakpoint once. Which engines a caller can run
//! (sitemap capture only Chromium, for one) is the caller's filter, after this one.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ProjectConfig, hidden_selection};

/// The breakpoints a run takes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(from = "ViewScopeRepr", into = "ViewScopeRepr")]
pub enum ViewScope {
    /// Every breakpoint of the config.
    #[default]
    All,
    /// The breakpoints the grid shows.
    Visible,
    /// The breakpoints named, by id.
    Breakpoints(Vec<String>),
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum ViewScopeRepr {
    Word(ScopeWord),
    Breakpoints(Vec<String>),
}

#[derive(Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum ScopeWord {
    All,
    Visible,
}

impl From<ViewScopeRepr> for ViewScope {
    fn from(repr: ViewScopeRepr) -> Self {
        match repr {
            ViewScopeRepr::Word(ScopeWord::All) => Self::All,
            ViewScopeRepr::Word(ScopeWord::Visible) => Self::Visible,
            ViewScopeRepr::Breakpoints(ids) => Self::Breakpoints(ids),
        }
    }
}

impl From<ViewScope> for ViewScopeRepr {
    fn from(scope: ViewScope) -> Self {
        match scope {
            ViewScope::All => Self::Word(ScopeWord::All),
            ViewScope::Visible => Self::Word(ScopeWord::Visible),
            ViewScope::Breakpoints(ids) => Self::Breakpoints(ids),
        }
    }
}

/// Why a scope gives no breakpoints; the code is an i18n key.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ScopeError {
    /// A named breakpoint the config does not have.
    #[error("scope.error.unknown_breakpoint: breakpoint `{0}` does not exist")]
    UnknownBreakpoint(String),
    /// The scope holds no breakpoint.
    #[error("scope.error.empty: the scope holds no breakpoint")]
    Empty,
}

impl ScopeError {
    /// The stable code (an i18n key).
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownBreakpoint(_) => "scope.error.unknown_breakpoint",
            Self::Empty => "scope.error.empty",
        }
    }
}

/// The breakpoint ids `scope` takes in `config`, in config order.
///
/// # Errors
///
/// [`ScopeError::UnknownBreakpoint`] for a named id the config lacks, [`ScopeError::Empty`] when
/// no breakpoint is left.
pub fn resolve_scope(config: &ProjectConfig, scope: &ViewScope) -> Result<Vec<String>, ScopeError> {
    let ids = config.breakpoints.iter().map(|breakpoint| &breakpoint.id);
    let chosen: Vec<String> = match scope {
        ViewScope::All => ids.cloned().collect(),
        ViewScope::Visible => {
            let hidden = hidden_selection(config);
            let cells = &config.layout.cells;
            ids.filter(|id| !hidden.contains(id))
                .filter(|id| cells.is_empty() || cells.iter().any(|cell| cell.breakpoint == **id))
                .cloned()
                .collect()
        }
        ViewScope::Breakpoints(named) => {
            if let Some(unknown) = named
                .iter()
                .find(|wanted| !config.breakpoints.iter().any(|bp| bp.id == **wanted))
            {
                return Err(ScopeError::UnknownBreakpoint(unknown.clone()));
            }
            ids.filter(|id| named.contains(id)).cloned().collect()
        }
    };
    if chosen.is_empty() {
        return Err(ScopeError::Empty);
    }
    Ok(chosen)
}

#[cfg(test)]
#[path = "view_scope_tests.rs"]
mod tests;
