use std::path::{Path, PathBuf};

use rb_ipc::McpClient;

use super::{Os, UserFiles, backup_path, read, write};

#[test]
fn the_user_files_lie_where_each_client_reads_them() {
    let home = Path::new("C:/Users/me");
    let app_data = Path::new("C:/Users/me/AppData/Roaming");
    let windows = UserFiles::locate(Os::Windows, Some(home), Some(app_data));
    assert_eq!(
        windows.of(McpClient::Cursor),
        Some(home.join(".cursor").join("mcp.json").as_path())
    );
    assert_eq!(
        windows.of(McpClient::ClaudeDesktop),
        Some(
            app_data
                .join("Claude")
                .join("claude_desktop_config.json")
                .as_path()
        )
    );
    let mac = UserFiles::locate(Os::MacOs, Some(Path::new("/Users/me")), None);
    assert_eq!(
        mac.of(McpClient::ClaudeDesktop),
        Some(Path::new(
            "/Users/me/Library/Application Support/Claude/claude_desktop_config.json"
        ))
    );
    let linux = UserFiles::locate(Os::Other, Some(Path::new("/home/me")), None);
    assert_eq!(
        linux.of(McpClient::ClaudeDesktop),
        Some(Path::new(
            "/home/me/.config/Claude/claude_desktop_config.json"
        ))
    );
    // The terminal clients are set up by their command, never by a file of theirs.
    for client in [
        McpClient::ClaudeCode,
        McpClient::Vscode,
        McpClient::Codex,
        McpClient::Pi,
    ] {
        assert_eq!(windows.of(client), None);
    }
    // Without the folders there is nothing to write.
    let none = UserFiles::locate(Os::Windows, None, None);
    assert_eq!(none.of(McpClient::Cursor), None);
    assert_eq!(none.of(McpClient::ClaudeDesktop), None);
}

#[test]
fn a_missing_file_reads_as_none() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    assert_eq!(
        read(&folder.path().join("none.json")).expect("no error"),
        None
    );
}

#[test]
fn a_user_file_is_backed_up_before_it_is_written() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder
        .path()
        .join("Claude")
        .join("claude_desktop_config.json");
    // A new file: its folder is made, there is nothing to keep.
    assert_eq!(
        write(&path, "{}\n", Some(1_700_000_000)).expect("written"),
        None
    );
    assert_eq!(read(&path).expect("read").as_deref(), Some("{}\n"));
    let backup = write(&path, "{\"a\":1}\n", Some(1_700_000_000))
        .expect("written")
        .expect("the old file is kept");
    assert_eq!(backup, backup_path(&path, 1_700_000_000));
    assert_eq!(
        std::fs::read_to_string(&backup).expect("the backup"),
        "{}\n"
    );
    assert_eq!(read(&path).expect("read").as_deref(), Some("{\"a\":1}\n"));
    // A second backup in the same second never overwrites the first.
    let second = write(&path, "{\"a\":2}\n", Some(1_700_000_000))
        .expect("written")
        .expect("kept again");
    assert_ne!(second, backup);
    assert_eq!(
        std::fs::read_to_string(&second).expect("the backup"),
        "{\"a\":1}\n"
    );
    assert_eq!(std::fs::read_to_string(&backup).expect("the first"), "{}\n");
}

#[test]
fn a_project_file_is_written_without_a_backup() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join(".cursor").join("mcp.json");
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("made");
    std::fs::write(&path, "{}").expect("an old file");
    assert_eq!(write(&path, "{\"b\":1}\n", None).expect("written"), None);
    let left: Vec<PathBuf> = std::fs::read_dir(path.parent().expect("a folder"))
        .expect("listed")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(left, [path]);
}

#[test]
fn the_backup_names_the_file_and_the_time() {
    assert_eq!(
        backup_path(Path::new("C:/x/mcp.json"), 42),
        Path::new("C:/x/mcp.json.rbrowser-42.bak")
    );
}
