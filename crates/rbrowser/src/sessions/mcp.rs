//! The MCP side of sessions (RB-020-07): the tools `session_assign` and `session_clear` as their
//! names, descriptions, arguments and answers. The MCP server (RB-030-01) registers them; each
//! runs the command of the same name with its arguments (the shortcut runs it without, and the
//! shell asks). Neither touches workspace trust: a session only separates cookies and storage.

use rb_engine::{Target, ViewId};
use serde::{Deserialize, Serialize};

/// The tool of `session.assign`.
pub const ASSIGN_TOOL: &str = "session_assign";
/// What `session_assign` does, for the tool list.
pub const ASSIGN_DESCRIPTION: &str = "Move views into a session (\"default\" or one of the \
     project's `sessions`): each view is created anew in the session's own cookie and storage \
     context and loads its current URL again. target: \"all\" or a view id; the active view \
     when absent.";
/// The tool of `session.clear`.
pub const CLEAR_TOOL: &str = "session_clear";
/// What `session_clear` does, for the tool list.
pub const CLEAR_DESCRIPTION: &str = "Clear a session's data: every cookie of the session, \
     and the storage (local and session storage, IndexedDB, cache storage, service workers) of \
     every origin its views show; the session's views reload. Other sessions keep theirs.";

/// The arguments of `session.assign` / `session_assign`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssignArgs {
    /// `"all"` or a view id; the active view when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// The session id.
    pub session: String,
}

/// One view `session.assign` moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Moved {
    /// The view before.
    pub from: ViewId,
    /// The view created in the session (it replaces `from` in its cell).
    pub view: ViewId,
}

/// The answer of `session.assign`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignAnswer {
    /// The session.
    pub session: String,
    /// The views moved; a view already in the session is not moved.
    pub moved: Vec<Moved>,
}

/// The arguments of `session.clear` / `session_clear`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClearArgs {
    /// The session id.
    pub session: String,
}

/// The answer of `session.clear`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearAnswer {
    /// The session.
    pub session: String,
    /// Its views, reloaded once the data is gone.
    pub views: Vec<ViewId>,
    /// The origins whose storage is cleared.
    pub origins: Vec<String>,
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
