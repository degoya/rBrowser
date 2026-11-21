use rb_ipc::McpFileStatus;
use serde_json::json;

use super::merge;

fn entry() -> serde_json::Value {
    json!({ "command": "C:\\Tools\\rBrowser\\rbrowser.exe", "args": ["mcp"] })
}

#[test]
fn a_missing_or_empty_file_is_created_with_the_entry_alone() {
    for existing in [None, Some(""), Some("  \n")] {
        let merged = merge(existing, &entry());
        assert_eq!(merged.status, McpFileStatus::Create);
        assert_eq!(merged.kept, 0);
        assert_eq!(
            merged.text.as_deref(),
            Some(include_str!("fixtures/claude-code.mcp.json"))
        );
    }
}

#[test]
fn an_existing_file_keeps_every_other_server_and_key() {
    let merged = merge(Some(include_str!("fixtures/merge-before.json")), &entry());
    assert_eq!(merged.status, McpFileStatus::Add);
    assert_eq!(merged.kept, 1);
    assert_eq!(
        merged.text.as_deref(),
        Some(include_str!("fixtures/merge-after.json"))
    );
}

#[test]
fn the_same_entry_writes_nothing() {
    let merged = merge(
        Some(include_str!("fixtures/claude-code.mcp.json")),
        &entry(),
    );
    assert_eq!(merged.status, McpFileStatus::Unchanged);
    assert_eq!(merged.text, None);
}

#[test]
fn another_rbrowser_entry_is_replaced_in_its_place_and_nothing_else() {
    let before = r#"{"mcpServers":{"rbrowser":{"command":"D:\\old\\rbrowser.exe","args":["mcp"]},"other":{"command":"x"}}}"#;
    let merged = merge(Some(before), &entry());
    assert_eq!(merged.status, McpFileStatus::Replace);
    assert_eq!(merged.kept, 1);
    let text = merged.text.expect("a replaced entry is written");
    let written: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(written["mcpServers"]["rbrowser"], entry());
    assert_eq!(written["mcpServers"]["other"], json!({ "command": "x" }));
    let order: Vec<&String> = written["mcpServers"]
        .as_object()
        .expect("an object")
        .keys()
        .collect();
    assert_eq!(order, ["rbrowser", "other"]);
}

#[test]
fn a_file_it_cannot_merge_is_left_alone() {
    for existing in [
        "{ // a comment\n}",
        "[1, 2]",
        r#"{"mcpServers": []}"#,
        "not json",
    ] {
        let merged = merge(Some(existing), &entry());
        assert_eq!(merged.status, McpFileStatus::Unreadable, "{existing}");
        assert_eq!(merged.text, None);
    }
}

#[test]
fn a_file_without_servers_gets_the_key() {
    let merged = merge(Some(r#"{"theme":"dark"}"#), &entry());
    assert_eq!(merged.status, McpFileStatus::Add);
    assert_eq!(merged.kept, 0);
    let written: serde_json::Value =
        serde_json::from_str(&merged.text.expect("written")).expect("JSON");
    assert_eq!(written["theme"], "dark");
    assert_eq!(written["mcpServers"]["rbrowser"], entry());
}
