//! What each AI client gets from the MCP setup (RB-091-09): its project file, its connection for
//! every project (a command to copy, or the user file the setup writes after the person
//! confirmed it), and what the person should know. The sources are the clients' own documents,
//! named in the job file:
//!
//! | Client | In the project | For every project |
//! | --- | --- | --- |
//! | Claude Code | `.mcp.json` | `claude mcp add --scope user` |
//! | VS Code / Copilot | `.mcp.json` (workspace root) | `code --add-mcp` |
//! | Claude Desktop | none | `claude_desktop_config.json` |
//! | Cursor | `.cursor/mcp.json` | `~/.cursor/mcp.json` |
//! | OpenAI Codex | `.codex/config.toml` (trusted projects) | `codex mcp add` |
//! | pi | `.pi/mcp.json` (trusted projects) | `pi mcp add` |
//!
//! Every way is stdio through `rbrowser mcp`: no port and no token in any file or line. In a
//! project the client starts it inside the project, so it finds that project's rBrowser; for
//! every project it finds the project the client works in, else the one rBrowser that runs.

pub mod files;
pub mod merge_json;
pub mod merge_toml;

use std::path::Path;

use rb_ipc::{McpClient, McpClientPlan, McpCommandLine, McpFilePlan, McpFileStatus, McpShell};
use serde_json::{Value, json};

/// The server's name in every client.
pub const SERVER_NAME: &str = "rbrowser";
/// The argument that makes `rbrowser` the stdio bridge.
pub const ARG: &str = "mcp";

/// Claude Desktop has no project configuration.
pub const NO_PROJECT_FILE_KEY: &str = "mcp.setup.reason.no_project_file";
/// No project is open and the wizard set none up.
pub const NO_PROJECT_KEY: &str = "mcp.setup.reason.no_project";

/// What a write does to one file, and the text it writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merged {
    /// What is done.
    pub status: McpFileStatus,
    /// The whole file as it is written; `None` when nothing is (unchanged, unreadable).
    pub text: Option<String>,
    /// The other servers in the file.
    pub kept: u32,
}

impl Merged {
    /// Nothing is written: the file cannot be merged.
    #[must_use]
    pub const fn unreadable() -> Self {
        Self {
            status: McpFileStatus::Unreadable,
            text: None,
            kept: 0,
        }
    }
}

/// A client's file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// `{ "mcpServers": { "rbrowser": { "command", "args" } } }`; `typed` adds
    /// `"type": "stdio"` (Cursor marks it required).
    Json {
        /// With `"type": "stdio"`.
        typed: bool,
    },
    /// Codex's `[mcp_servers.rbrowser]`.
    Toml,
}

impl Format {
    /// The `rbrowser` entry for `command`, as it is written.
    #[must_use]
    pub fn entry(self, command: &str) -> String {
        match self {
            Self::Json { typed } => {
                serde_json::to_string_pretty(&json_entry(typed, command)).unwrap_or_default()
            }
            Self::Toml => merge_toml::entry(command),
        }
    }

    /// The entry for `command` merged into `existing`.
    #[must_use]
    pub fn merge(self, existing: Option<&str>, command: &str) -> Merged {
        match self {
            Self::Json { typed } => merge_json::merge(existing, &json_entry(typed, command)),
            Self::Toml => merge_toml::merge(existing, &merge_toml::entry(command)),
        }
    }
}

fn json_entry(typed: bool, command: &str) -> Value {
    if typed {
        json!({ "type": "stdio", "command": command, "args": [ARG] })
    } else {
        json!({ "command": command, "args": [ARG] })
    }
}

/// The client's project file, relative to the project, and its format.
#[must_use]
pub const fn project_file(client: McpClient) -> Option<(&'static str, Format)> {
    match client {
        // VS Code's native workspace file is the same `.mcp.json` (the editor's own
        // `.vscode/mcp.json` is for its Local harness, which is deprecated).
        McpClient::ClaudeCode | McpClient::Vscode => {
            Some((".mcp.json", Format::Json { typed: false }))
        }
        McpClient::Cursor => Some((".cursor/mcp.json", Format::Json { typed: true })),
        McpClient::Codex => Some((".codex/config.toml", Format::Toml)),
        McpClient::Pi => Some((".pi/mcp.json", Format::Json { typed: false })),
        McpClient::ClaudeDesktop => None,
    }
}

/// The format of the client's user file, for a client that is set up by file only.
#[must_use]
pub const fn user_file_format(client: McpClient) -> Option<Format> {
    match client {
        McpClient::Cursor => Some(Format::Json { typed: true }),
        McpClient::ClaudeDesktop => Some(Format::Json { typed: false }),
        _ => None,
    }
}

/// What the person should know about the client (an i18n key).
#[must_use]
pub const fn note(client: McpClient) -> Option<&'static str> {
    match client {
        McpClient::ClaudeCode => None,
        McpClient::Vscode => Some("mcp.setup.note.vscode"),
        McpClient::ClaudeDesktop => Some("mcp.setup.note.claude_desktop"),
        McpClient::Cursor => Some("mcp.setup.note.cursor"),
        McpClient::Codex => Some("mcp.setup.note.codex"),
        McpClient::Pi => Some("mcp.setup.note.pi"),
    }
}

/// The line that adds rBrowser to the client for every project, starting `command`; `None` for a
/// client set up by its user file.
#[must_use]
pub fn command_line(client: McpClient, command: &str) -> Option<String> {
    let add = |tool: &str, scope: &str| {
        format!("{tool} mcp add {scope}{SERVER_NAME} -- \"{command}\" {ARG}")
    };
    match client {
        McpClient::ClaudeCode => Some(add("claude", "--scope user ")),
        McpClient::Codex => Some(add("codex", "")),
        McpClient::Pi => Some(add("pi", "")),
        McpClient::Vscode => {
            let server =
                json!({ "name": SERVER_NAME, "type": "stdio", "command": command, "args": [ARG] });
            Some(format!("code --add-mcp '{server}'"))
        }
        McpClient::Cursor | McpClient::ClaudeDesktop => None,
    }
}

/// The client's lines: for the program at `exe` and, when it is reachable from WSL, at
/// `wsl_exe` - for the terminal clients only (VS Code adds to the editor's profile on Windows).
#[must_use]
pub fn commands(client: McpClient, exe: &str, wsl_exe: Option<&str>) -> Vec<McpCommandLine> {
    let Some(line) = command_line(client, exe) else {
        return Vec::new();
    };
    let terminal = matches!(
        client,
        McpClient::ClaudeCode | McpClient::Codex | McpClient::Pi
    );
    match wsl_exe.filter(|_| terminal) {
        Some(wsl) => vec![
            McpCommandLine {
                shell: McpShell::Windows,
                line,
            },
            McpCommandLine {
                shell: McpShell::Wsl,
                line: command_line(client, wsl).unwrap_or_default(),
            },
        ],
        None => vec![McpCommandLine {
            shell: if wsl_exe.is_some() {
                McpShell::Windows
            } else {
                McpShell::Any
            },
            line,
        }],
    }
}

/// What the plan is made of: the program as the project's client starts it, the program for
/// every project (and from WSL), the project, the user files.
#[derive(Clone, Debug)]
pub struct PlanInput<'a> {
    /// The program as the project file names it: from WSL when the project lies in WSL.
    pub project_command: &'a str,
    /// The program on Windows (or this system).
    pub exe: &'a str,
    /// The program from WSL; `None` when WSL cannot reach it.
    pub wsl_exe: Option<&'a str>,
    /// The project folder; `None` without one.
    pub root: Option<&'a Path>,
    /// Where each client's user file is.
    pub user_files: &'a files::UserFiles,
}

/// The plan for `client`; `read` gives a file's text (`None` when there is none).
pub fn plan(
    client: McpClient,
    input: &PlanInput<'_>,
    read: &dyn Fn(&Path) -> std::io::Result<Option<String>>,
) -> McpClientPlan {
    let (project, project_reason) = match (project_file(client), input.root) {
        (None, _) => (None, Some(NO_PROJECT_FILE_KEY.to_owned())),
        (Some(_), None) => (None, Some(NO_PROJECT_KEY.to_owned())),
        (Some((relative, format)), Some(root)) => {
            let path = root.join(relative);
            let plan = file_plan(relative, format, input.project_command, read(&path));
            (Some(plan), None)
        }
    };
    let user_file = user_file_format(client)
        .zip(input.user_files.of(client))
        .map(|(format, path)| {
            let shown = path.to_string_lossy();
            file_plan(&shown, format, input.exe, read(path))
        });
    McpClientPlan {
        client,
        project,
        project_reason,
        user_file,
        commands: commands(client, input.exe, input.wsl_exe),
        note: note(client).map(str::to_owned),
    }
}

fn file_plan(
    shown: &str,
    format: Format,
    command: &str,
    existing: std::io::Result<Option<String>>,
) -> McpFilePlan {
    let merged = match &existing {
        Ok(text) => format.merge(text.as_deref(), command),
        Err(_) => Merged::unreadable(),
    };
    McpFilePlan {
        path: shown.to_owned(),
        status: merged.status,
        entry: format.entry(command),
        kept: merged.kept,
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
