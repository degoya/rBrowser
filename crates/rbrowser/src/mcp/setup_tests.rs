use std::path::{Path, PathBuf};

use rb_ipc::{McpClient, McpFileStatus};

use super::{
    Machine, NOTHING_KEY, ROOT_REFUSED_KEY, UNREADABLE_KEY, is_request, pick_root, project_command,
    write,
};
use crate::mcp::clients::files::{Os, UserFiles};
use crate::mcp::clients::{NO_PROJECT_FILE_KEY, NO_PROJECT_KEY};

const EXE: &str = "C:\\Tools\\rBrowser\\rbrowser.exe";
const WSL_EXE: &str = "/mnt/c/Tools/rBrowser/rbrowser.exe";

fn machine(home: &Path) -> Machine {
    Machine {
        exe: EXE.to_owned(),
        wsl_exe: Some(WSL_EXE.to_owned()),
        user_files: UserFiles::locate(
            Os::Windows,
            Some(home),
            Some(home.join("AppData").as_path()),
        ),
    }
}

#[test]
fn only_the_open_project_or_the_one_the_wizard_set_up_is_written_into() {
    let open = PathBuf::from("C:/work/shop");
    let set_up = PathBuf::from("C:/work/blog");
    let allowed = [open.clone(), set_up.clone()];
    assert_eq!(pick_root(None, &allowed), Ok(Some(open.clone())));
    assert_eq!(
        pick_root(Some("c:\\work\\blog\\"), &allowed),
        Ok(Some(set_up))
    );
    assert_eq!(
        pick_root(Some("C:/Windows"), &allowed).map_err(|failure| failure.0),
        Err(ROOT_REFUSED_KEY)
    );
    assert_eq!(pick_root(None, &[]), Ok(None));
    assert!(pick_root(Some("C:/work/shop"), &[]).is_err());
}

#[test]
fn a_project_in_wsl_starts_rbrowser_by_its_wsl_path() {
    let home = Path::new("C:/Users/me");
    let here = machine(home);
    assert_eq!(
        project_command(
            &here,
            Some(Path::new(r"\\wsl.localhost\Ubuntu\home\me\site"))
        ),
        WSL_EXE
    );
    assert_eq!(project_command(&here, Some(Path::new("C:/work/shop"))), EXE);
    assert_eq!(project_command(&here, None), EXE);
}

#[test]
fn the_project_file_is_written_beside_the_servers_already_there() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let root = folder.path().join("shop");
    std::fs::create_dir_all(&root).expect("a project");
    std::fs::write(
        root.join(".mcp.json"),
        include_str!("clients/fixtures/merge-before.json"),
    )
    .expect("a file of the team");
    let here = machine(&folder.path().join("home"));
    let files = write(McpClient::ClaudeCode, Some(&root), &here, true, false, 7).expect("written");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, ".mcp.json");
    assert_eq!(files[0].status, McpFileStatus::Add);
    assert_eq!(files[0].backup, None);
    assert_eq!(
        std::fs::read_to_string(root.join(".mcp.json")).expect("read"),
        include_str!("clients/fixtures/merge-after.json")
    );
    // Again: nothing changes.
    let again = write(McpClient::ClaudeCode, Some(&root), &here, true, false, 8).expect("written");
    assert_eq!(again[0].status, McpFileStatus::Unchanged);
}

#[test]
fn a_file_it_cannot_merge_is_never_written() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let root = folder.path().to_path_buf();
    std::fs::create_dir_all(root.join(".cursor")).expect("a folder");
    let before = "{ // the person's comment\n \"mcpServers\": {} }";
    std::fs::write(root.join(".cursor/mcp.json"), before).expect("a file");
    let here = machine(&folder.path().join("home"));
    let failure =
        write(McpClient::Cursor, Some(&root), &here, true, false, 7).expect_err("refused");
    assert_eq!(failure.0, UNREADABLE_KEY);
    assert_eq!(
        std::fs::read_to_string(root.join(".cursor/mcp.json")).expect("read"),
        before
    );
}

#[test]
fn a_user_file_keeps_a_backup_of_the_old_one() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let home = folder.path().join("home");
    let here = machine(&home);
    let path = home.join("AppData/Claude/claude_desktop_config.json");
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("made");
    std::fs::write(&path, include_str!("clients/fixtures/merge-before.json")).expect("a file");
    let files = write(
        McpClient::ClaudeDesktop,
        None,
        &here,
        false,
        true,
        1_700_000_000,
    )
    .expect("written");
    assert_eq!(files[0].status, McpFileStatus::Add);
    let backup = files[0].backup.clone().expect("a backup");
    assert_eq!(
        std::fs::read_to_string(&backup).expect("the backup"),
        include_str!("clients/fixtures/merge-before.json")
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        include_str!("clients/fixtures/merge-after.json")
    );
}

#[test]
fn a_write_names_what_is_missing() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let here = machine(&folder.path().join("home"));
    let root = folder.path().to_path_buf();
    let nothing = write(McpClient::ClaudeCode, Some(&root), &here, false, false, 1);
    assert_eq!(nothing.expect_err("nothing").0, NOTHING_KEY);
    // Claude Code is set up for every project by its command, never by a file of its own.
    let no_user_file = write(McpClient::ClaudeCode, Some(&root), &here, false, true, 1);
    assert_eq!(no_user_file.expect_err("no file").0, NOTHING_KEY);
    let desktop = write(McpClient::ClaudeDesktop, Some(&root), &here, true, false, 1);
    assert_eq!(desktop.expect_err("no project file").0, NO_PROJECT_FILE_KEY);
    let no_root = write(McpClient::Codex, None, &here, true, false, 1);
    assert_eq!(no_root.expect_err("no project").0, NO_PROJECT_KEY);
}

#[test]
fn only_mcp_setup_messages_are_its_requests() {
    assert!(is_request(
        r#"{"type":"mcpSetup","request":{"op":"check","requestId":1}}"#
    ));
    assert!(!is_request(r#"{"type":"getAbout"}"#));
}
