use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rb_ipc::{McpClient, McpFileStatus, McpShell};

use super::files::{Os, UserFiles};
use super::{
    Format, NO_PROJECT_FILE_KEY, NO_PROJECT_KEY, PlanInput, command_line, commands, plan,
    project_file,
};

const EXE: &str = "C:\\Tools\\rBrowser\\rbrowser.exe";
const WSL_EXE: &str = "/mnt/c/Tools/rBrowser/rbrowser.exe";

fn user_files() -> UserFiles {
    UserFiles::locate(
        Os::Windows,
        Some(Path::new("C:/Users/me")),
        Some(Path::new("C:/Users/me/AppData/Roaming")),
    )
}

fn input<'a>(root: Option<&'a Path>, files: &'a UserFiles) -> PlanInput<'a> {
    PlanInput {
        project_command: EXE,
        exe: EXE,
        wsl_exe: None,
        root,
        user_files: files,
    }
}

/// A disk with these files only.
fn disk(files: BTreeMap<PathBuf, String>) -> impl Fn(&Path) -> std::io::Result<Option<String>> {
    move |path| Ok(files.get(path).cloned())
}

#[test]
fn each_client_writes_its_own_project_file_exactly_as_its_fixture() {
    let cases = [
        (
            McpClient::ClaudeCode,
            ".mcp.json",
            include_str!("fixtures/claude-code.mcp.json"),
        ),
        (
            McpClient::Vscode,
            ".mcp.json",
            include_str!("fixtures/claude-code.mcp.json"),
        ),
        (
            McpClient::Cursor,
            ".cursor/mcp.json",
            include_str!("fixtures/cursor.mcp.json"),
        ),
        (
            McpClient::Codex,
            ".codex/config.toml",
            include_str!("fixtures/codex.config.toml"),
        ),
        (
            McpClient::Pi,
            ".pi/mcp.json",
            include_str!("fixtures/claude-code.mcp.json"),
        ),
    ];
    for (client, path, fixture) in cases {
        let (relative, format) = project_file(client).expect("a project file");
        assert_eq!(relative, path, "{client:?}");
        assert_eq!(
            format.merge(None, EXE).text.as_deref(),
            Some(fixture),
            "{client:?}"
        );
    }
    assert_eq!(project_file(McpClient::ClaudeDesktop), None);
}

#[test]
fn the_plan_shows_the_project_file_its_entry_and_what_a_write_does() {
    let files = user_files();
    let root = Path::new("C:/work/shop");
    let existing = BTreeMap::from([(
        root.join(".mcp.json"),
        include_str!("fixtures/merge-before.json").to_owned(),
    )]);
    let planned = plan(
        McpClient::ClaudeCode,
        &input(Some(root), &files),
        &disk(existing),
    );
    let project = planned.project.expect("a project file");
    assert_eq!(project.path, ".mcp.json");
    assert_eq!(project.status, McpFileStatus::Add);
    assert_eq!(project.kept, 1);
    assert_eq!(project.entry, Format::Json { typed: false }.entry(EXE));
    // Another server's entry - its secret included - is never in the plan.
    assert!(!project.entry.contains("kept-as-it-is"));
    assert!(!project.entry.contains("filesystem"));
    assert_eq!(planned.project_reason, None);
    assert_eq!(planned.user_file, None);
    assert_eq!(planned.commands.len(), 1);
    assert_eq!(planned.commands[0].shell, McpShell::Any);
}

#[test]
fn without_a_project_only_the_way_for_every_project_is_offered() {
    let files = user_files();
    let planned = plan(
        McpClient::Codex,
        &input(None, &files),
        &disk(BTreeMap::new()),
    );
    assert_eq!(planned.project, None);
    assert_eq!(planned.project_reason.as_deref(), Some(NO_PROJECT_KEY));
    assert_eq!(
        planned.commands[0].line,
        format!("codex mcp add rbrowser -- \"{EXE}\" mcp")
    );
    assert_eq!(planned.note.as_deref(), Some("mcp.setup.note.codex"));
}

#[test]
fn claude_desktop_has_no_project_file_and_its_user_file_instead() {
    let files = user_files();
    let planned = plan(
        McpClient::ClaudeDesktop,
        &input(Some(Path::new("C:/work/shop")), &files),
        &disk(BTreeMap::new()),
    );
    assert_eq!(planned.project, None);
    assert_eq!(planned.project_reason.as_deref(), Some(NO_PROJECT_FILE_KEY));
    let user = planned.user_file.expect("its user file");
    assert!(
        user.path.ends_with("claude_desktop_config.json"),
        "{}",
        user.path
    );
    assert_eq!(user.status, McpFileStatus::Create);
    assert!(planned.commands.is_empty());
}

#[test]
fn cursor_has_both_files_and_a_file_that_cannot_be_read_is_never_written() {
    let files = user_files();
    let read = |path: &Path| -> std::io::Result<Option<String>> {
        if path.ends_with("mcp.json") && path.starts_with("C:/Users/me") {
            Err(std::io::Error::other("denied"))
        } else {
            Ok(None)
        }
    };
    let planned = plan(
        McpClient::Cursor,
        &input(Some(Path::new("C:/work/shop")), &files),
        &read,
    );
    assert_eq!(
        planned.project.expect("a project file").status,
        McpFileStatus::Create
    );
    assert_eq!(
        planned.user_file.expect("a user file").status,
        McpFileStatus::Unreadable
    );
}

#[test]
fn the_lines_for_every_project_name_no_port_and_no_token() {
    assert_eq!(
        command_line(McpClient::ClaudeCode, EXE).as_deref(),
        Some("claude mcp add --scope user rbrowser -- \"C:\\Tools\\rBrowser\\rbrowser.exe\" mcp")
    );
    assert_eq!(
        command_line(McpClient::Pi, WSL_EXE).as_deref(),
        Some("pi mcp add rbrowser -- \"/mnt/c/Tools/rBrowser/rbrowser.exe\" mcp")
    );
    assert_eq!(
        command_line(McpClient::Vscode, EXE).as_deref(),
        Some(
            "code --add-mcp '{\"name\":\"rbrowser\",\"type\":\"stdio\",\"command\":\"C:\\\\Tools\\\\rBrowser\\\\rbrowser.exe\",\"args\":[\"mcp\"]}'"
        )
    );
    assert_eq!(command_line(McpClient::Cursor, EXE), None);
    assert_eq!(command_line(McpClient::ClaudeDesktop, EXE), None);
    for client in McpClient::ALL {
        let line = command_line(client, EXE).unwrap_or_default();
        assert!(!line.contains("Bearer") && !line.contains("token") && !line.contains("http"));
    }
}

#[test]
fn a_terminal_client_gets_the_wsl_line_beside_the_windows_one() {
    let lines = commands(McpClient::ClaudeCode, EXE, Some(WSL_EXE));
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].shell, McpShell::Windows);
    assert_eq!(lines[1].shell, McpShell::Wsl);
    assert!(lines[1].line.contains(WSL_EXE));
    // VS Code adds to the editor's profile on Windows: one line.
    let vscode = commands(McpClient::Vscode, EXE, Some(WSL_EXE));
    assert_eq!(vscode.len(), 1);
    assert_eq!(vscode[0].shell, McpShell::Windows);
    assert!(commands(McpClient::Cursor, EXE, Some(WSL_EXE)).is_empty());
}

#[test]
fn a_project_in_wsl_names_the_program_by_its_wsl_path() {
    let files = user_files();
    let root = Path::new("//wsl.localhost/Ubuntu/home/me/site");
    let planned = plan(
        McpClient::ClaudeCode,
        &PlanInput {
            project_command: WSL_EXE,
            exe: EXE,
            wsl_exe: Some(WSL_EXE),
            root: Some(root),
            user_files: &files,
        },
        &disk(BTreeMap::new()),
    );
    let project = planned.project.expect("a project file");
    assert!(project.entry.contains(WSL_EXE), "{}", project.entry);
}
