use rb_ipc::McpFileStatus;

use super::{entry, merge};

const EXE: &str = "C:\\Tools\\rBrowser\\rbrowser.exe";

#[test]
fn the_entry_is_a_table_of_its_own_with_the_path_escaped() {
    assert_eq!(entry(EXE), include_str!("fixtures/codex.config.toml"));
    assert_eq!(
        entry("/mnt/c/a \"b\"/rbrowser.exe"),
        "[mcp_servers.rbrowser]\ncommand = \"/mnt/c/a \\\"b\\\"/rbrowser.exe\"\nargs = [\"mcp\"]\n"
    );
}

#[test]
fn a_missing_file_is_created_with_the_entry_alone() {
    let merged = merge(None, &entry(EXE));
    assert_eq!(merged.status, McpFileStatus::Create);
    assert_eq!(
        merged.text.as_deref(),
        Some(include_str!("fixtures/codex.config.toml"))
    );
}

#[test]
fn an_existing_file_keeps_its_comments_settings_and_servers() {
    let merged = merge(
        Some(include_str!("fixtures/codex-before.toml")),
        &entry(EXE),
    );
    assert_eq!(merged.status, McpFileStatus::Add);
    assert_eq!(merged.kept, 1);
    assert_eq!(
        merged.text.as_deref(),
        Some(include_str!("fixtures/codex-after.toml"))
    );
}

#[test]
fn the_same_entry_writes_nothing() {
    let merged = merge(Some(include_str!("fixtures/codex-after.toml")), &entry(EXE));
    assert_eq!(merged.status, McpFileStatus::Unchanged);
    assert_eq!(merged.kept, 1);
    assert_eq!(merged.text, None);
}

#[test]
fn another_rbrowser_table_is_replaced_with_its_sub_tables_and_nothing_else() {
    let merged = merge(
        Some(include_str!("fixtures/codex-replace-before.toml")),
        &entry(EXE),
    );
    assert_eq!(merged.status, McpFileStatus::Replace);
    assert_eq!(merged.kept, 1);
    assert_eq!(
        merged.text.as_deref(),
        Some(include_str!("fixtures/codex-replace-after.toml"))
    );
}

#[test]
fn a_file_without_a_final_newline_still_gets_a_blank_line_before_the_entry() {
    let merged = merge(Some("model = \"x\""), &entry(EXE));
    assert_eq!(merged.status, McpFileStatus::Add);
    assert_eq!(
        merged.text.as_deref(),
        Some(format!("model = \"x\"\n\n{}", entry(EXE)).as_str())
    );
}

#[test]
fn a_form_it_cannot_merge_is_left_alone() {
    for existing in [
        "mcp_servers = { rbrowser = { command = \"x\" } }\n",
        "mcp_servers.rbrowser.command = \"x\"\n",
        "[mcp_servers]\nrbrowser = { command = \"x\" }\n",
        "[mcp_servers]\nrbrowser.command = \"x\"\n",
    ] {
        let merged = merge(Some(existing), &entry(EXE));
        assert_eq!(merged.status, McpFileStatus::Unreadable, "{existing}");
        assert_eq!(merged.text, None);
    }
}

#[test]
fn quoted_headers_name_the_same_table() {
    let existing = "[mcp_servers.\"rbrowser\"]\ncommand = \"D:\\\\old.exe\"\n";
    let merged = merge(Some(existing), &entry(EXE));
    assert_eq!(merged.status, McpFileStatus::Replace);
    assert_eq!(merged.text.as_deref(), Some(entry(EXE).as_str()));
}
