//! The MCP setup's messages (RB-091-09): the setup wizard's step "Connect an AI agent" and
//! Settings > Automation > MCP send `{ "type": "mcpSetup", "request": { "op": ... } }` from the
//! pages window only; each request carries the page's `requestId`, is answered with `ack`, and
//! its result follows as a `mcpSetup` event with the same number (the files may lie in WSL).
//!
//! `plan` names, per client, the project file and the connection for every project - nothing
//! is written; `write` writes what the person ticked (a user file only with `user: true`, the
//! person's confirmation); `check` says whether the server runs and when a client last reached
//! it. No answer ever carries a token, and no file plan shows another server's entry.

use std::collections::BTreeMap;

pub use rb_config::McpClient;
use serde::{Deserialize, Serialize};

/// A request of the MCP setup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum McpSetupRequest {
    /// What every client would get for the project at `root` (else the open project).
    Plan {
        /// The page's number of this request; its answer carries it back.
        request_id: u32,
        /// The project folder; the open project or the one the setup wizard set up only.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root: Option<String>,
    },
    /// Writes `client`'s project file (`project`) and its user file (`user`, after the person
    /// confirmed it; a backup of the old file is kept).
    Write {
        /// The page's number of this request; its answer carries it back.
        request_id: u32,
        /// The client.
        client: McpClient,
        /// The project folder, as in `plan`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root: Option<String>,
        /// Write the project file.
        project: bool,
        /// Write the user file.
        user: bool,
    },
    /// Whether the server runs and when a client last reached it.
    Check {
        /// The page's number of this request; its answer carries it back.
        request_id: u32,
    },
}

impl McpSetupRequest {
    /// The page's number of this request.
    #[must_use]
    pub const fn request_id(&self) -> u32 {
        match self {
            Self::Plan { request_id, .. }
            | Self::Write { request_id, .. }
            | Self::Check { request_id } => *request_id,
        }
    }
}

/// What a write would do to a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpFileStatus {
    /// The file does not exist; it is created with the `rbrowser` entry alone.
    Create,
    /// The file exists; the `rbrowser` entry is added beside the others.
    Add,
    /// The file has this very `rbrowser` entry; nothing is written.
    Unchanged,
    /// The file has another `rbrowser` entry; only that entry is replaced.
    Replace,
    /// The file cannot be read or merged (not JSON, comments, an unusual form): nothing is
    /// written, the entry is added by hand.
    Unreadable,
}

/// A file the setup can write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpFilePlan {
    /// The file: relative to the project for a project file, absolute for a user file.
    pub path: String,
    /// What a write would do.
    pub status: McpFileStatus,
    /// The `rbrowser` entry as it is written (JSON or TOML).
    pub entry: String,
    /// The other servers in the file, which stay as they are.
    pub kept: u32,
}

/// Where a command line runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpShell {
    /// A terminal on this system.
    Any,
    /// A terminal on Windows.
    Windows,
    /// A terminal in WSL, rBrowser run through interop.
    Wsl,
}

/// A command that adds rBrowser to a client for every project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCommandLine {
    /// Where it runs.
    pub shell: McpShell,
    /// The line to copy.
    pub line: String,
}

/// One client's two ways.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientPlan {
    /// The client.
    pub client: McpClient,
    /// Its project file; absent without a project or when the client has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<McpFilePlan>,
    /// Why there is no project file (an i18n key), when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_reason: Option<String>,
    /// Its user file, for a client that is set up by file only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_file: Option<McpFilePlan>,
    /// The commands that add rBrowser for every project.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<McpCommandLine>,
    /// What the person should know about this client (an i18n key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A file `write` wrote.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpWrittenFile {
    /// The file.
    pub path: String,
    /// What was done.
    pub status: McpFileStatus,
    /// The copy of the old file, when there was one to keep.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
}

/// An answer of the MCP setup, as a `mcpSetup` event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum McpSetupAnswer {
    /// The answer to `plan`.
    Plan {
        /// The request it answers.
        request_id: u32,
        /// `mcp.enabled`.
        enabled: bool,
        /// The project folder the plan is for; absent without a project.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root: Option<String>,
        /// Every client, in [`McpClient::ALL`]'s order.
        clients: Vec<McpClientPlan>,
    },
    /// `write` wrote these files.
    Written {
        /// The request it answers.
        request_id: u32,
        /// The files, the project file first.
        files: Vec<McpWrittenFile>,
    },
    /// The answer to `check`.
    Check {
        /// The request it answers.
        request_id: u32,
        /// `mcp.enabled`.
        enabled: bool,
        /// The server listens.
        running: bool,
        /// Its port while it runs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port: Option<u16>,
        /// Seconds since a client last reached it; absent when none has since it started.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_call_secs: Option<u64>,
        /// How many requests clients sent since it started.
        calls: u64,
    },
    /// A request failed: the i18n key of the reason and its values.
    Failed {
        /// The request it answers.
        request_id: u32,
        /// The reason's i18n key.
        message_key: String,
        /// Values for the reason's placeholders.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        args: BTreeMap<String, String>,
    },
}

#[cfg(test)]
#[path = "mcp_setup_tests.rs"]
mod tests;
