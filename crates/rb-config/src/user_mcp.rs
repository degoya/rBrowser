//! `mcp` of the user settings: the MCP server (RB-030-01), moved out of `user_settings.rs` and
//! re-exported there. Every key is protected from MCP itself.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `mcp`. Protected from MCP itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct McpSettings {
    /// The MCP server runs.
    pub enabled: bool,
    /// The `evaluate` tool may run page JavaScript.
    pub allow_evaluate: bool,
    /// A fixed port on 127.0.0.1; a free one at every start when absent (RB-030-01).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// How long MCP waits for each view, in milliseconds: the final URL of `navigate`,
    /// `reload`, `back` and `forward`, the input of `click`, `type`, `hover` and `press_key`,
    /// and `sync_set`'s work (RB-030-01).
    #[schemars(range(min = 1000, max = 600_000))]
    pub navigation_timeout_ms: u32,
    /// Console messages kept per view for `console_logs` (RB-030-01).
    #[schemars(range(min = 10, max = 10_000))]
    pub console_log_limit: u32,
    /// Network requests kept per view for `network_requests` (RB-030-01).
    #[schemars(range(min = 10, max = 10_000))]
    pub network_request_limit: u32,
    /// Uncaught exceptions kept per view for `errors` (RB-030-01).
    #[schemars(range(min = 10, max = 10_000))]
    pub error_limit: u32,
    /// Network requests are collected from the start; off, from the first time they are asked
    /// for (`network_requests`, a plugin's network list) (RB-090-34).
    pub collect_network: bool,
    /// The most bytes `evaluate` and `snapshot` answer; more is cut (RB-030-01).
    #[schemars(range(min = 4096, max = 16_777_216))]
    pub max_answer_bytes: u32,
    /// `rbrowser mcp` starts the project's rBrowser in the background when none runs
    /// (RB-030-02).
    pub stdio_auto_start: bool,
    /// A setting changed over MCP is shown in the shell (RB-030-03).
    pub notify_on_setting_change: bool,
    /// The server offers its prompts, the workflows `responsive_review`, `a11y_triage` and
    /// `bug_repro` (RB-095-01).
    pub prompts: bool,
    /// The server offers its read-only resources: `rbrowser://views`, the latest findings and
    /// screenshots, the project config (RB-095-01).
    pub resources: bool,
    /// The AI client the MCP setup shows first, in the setup wizard and in Settings >
    /// Automation > MCP (RB-091-09).
    pub client: McpClient,
}

/// The AI clients the MCP setup knows (RB-091-09): each one's project file and its connection
/// for every project are `rbrowser::mcp::clients`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum McpClient {
    /// Claude Code.
    #[default]
    ClaudeCode,
    /// VS Code with GitHub Copilot.
    Vscode,
    /// Claude Desktop.
    ClaudeDesktop,
    /// Cursor.
    Cursor,
    /// OpenAI Codex.
    Codex,
    /// The pi coding agent.
    Pi,
}

impl McpClient {
    /// Every client, in the order the setup lists them.
    pub const ALL: [Self; 6] = [
        Self::ClaudeCode,
        Self::Vscode,
        Self::ClaudeDesktop,
        Self::Cursor,
        Self::Codex,
        Self::Pi,
    ];

    /// The client's id, as the settings and the shell name it (`claudeCode`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claudeCode",
            Self::Vscode => "vscode",
            Self::ClaudeDesktop => "claudeDesktop",
            Self::Cursor => "cursor",
            Self::Codex => "codex",
            Self::Pi => "pi",
        }
    }
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_evaluate: false,
            port: None,
            navigation_timeout_ms: 15_000,
            console_log_limit: 500,
            network_request_limit: 500,
            error_limit: 200,
            collect_network: false,
            max_answer_bytes: 262_144,
            stdio_auto_start: false,
            notify_on_setting_change: true,
            prompts: true,
            resources: true,
            client: McpClient::ClaudeCode,
        }
    }
}

#[cfg(test)]
#[path = "user_mcp_tests.rs"]
mod tests;
