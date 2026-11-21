use serde_json::json;

use super::*;
use crate::{CoreToShell, ShellToCore, parse_request};

#[test]
fn a_write_request_reads_in_camel_case_under_mcp_setup() {
    let request = parse_request(
        &json!({
            "type": "mcpSetup",
            "request": {
                "op": "write",
                "requestId": 4,
                "client": "cursor",
                "root": "C:/work/shop",
                "project": true,
                "user": false
            }
        })
        .to_string(),
    )
    .expect("parses");
    assert_eq!(
        request,
        ShellToCore::McpSetup {
            request: McpSetupRequest::Write {
                request_id: 4,
                client: McpClient::Cursor,
                root: Some("C:/work/shop".to_owned()),
                project: true,
                user: false,
            },
        }
    );
}

#[test]
fn a_plan_without_a_root_and_a_check_read() {
    let plan = parse_request(r#"{"type":"mcpSetup","request":{"op":"plan","requestId":1}}"#)
        .expect("parses");
    assert_eq!(
        plan,
        ShellToCore::McpSetup {
            request: McpSetupRequest::Plan {
                request_id: 1,
                root: None,
            },
        }
    );
    let check = parse_request(r#"{"type":"mcpSetup","request":{"op":"check","requestId":2}}"#)
        .expect("parses");
    assert_eq!(
        check,
        ShellToCore::McpSetup {
            request: McpSetupRequest::Check { request_id: 2 },
        }
    );
    assert_eq!(
        McpSetupRequest::Check { request_id: 2 }.request_id(),
        2,
        "every request names its number"
    );
}

#[test]
fn an_unknown_client_is_refused() {
    let request = r#"{"type":"mcpSetup","request":{"op":"write","requestId":4,"client":"emacs","project":true,"user":false}}"#;
    assert!(parse_request(request).is_err());
}

#[test]
fn a_plan_answer_writes_its_files_and_commands_in_camel_case() {
    let answer = CoreToShell::McpSetup {
        answer: McpSetupAnswer::Plan {
            request_id: 1,
            enabled: false,
            root: Some("C:/work/shop".to_owned()),
            clients: vec![McpClientPlan {
                client: McpClient::ClaudeDesktop,
                project: None,
                project_reason: Some("mcp.setup.reason.no_project_file".to_owned()),
                user_file: Some(McpFilePlan {
                    path: "C:/Users/me/AppData/Roaming/Claude/claude_desktop_config.json"
                        .to_owned(),
                    status: McpFileStatus::Add,
                    entry: "{}".to_owned(),
                    kept: 2,
                }),
                commands: vec![McpCommandLine {
                    shell: McpShell::Any,
                    line: "pi mcp add rbrowser -- rbrowser mcp".to_owned(),
                }],
                note: None,
            }],
        },
    };
    let written = serde_json::to_value(&answer).expect("serialises");
    assert_eq!(written["type"], "mcpSetup");
    assert_eq!(written["answer"]["op"], "plan");
    let client = &written["answer"]["clients"][0];
    assert_eq!(client["client"], "claudeDesktop");
    assert_eq!(client["projectReason"], "mcp.setup.reason.no_project_file");
    assert_eq!(client["userFile"]["status"], "add");
    assert_eq!(client["userFile"]["kept"], 2);
    assert_eq!(client["commands"][0]["shell"], "any");
    assert!(client.get("project").is_none());
    assert!(client.get("note").is_none());
    let back: CoreToShell = serde_json::from_value(written).expect("reads back");
    assert_eq!(back, answer);
}

#[test]
fn a_check_answer_names_the_port_and_the_last_call_but_no_token() {
    let answer = McpSetupAnswer::Check {
        request_id: 3,
        enabled: true,
        running: true,
        port: Some(7420),
        last_call_secs: Some(12),
        calls: 5,
    };
    let written = serde_json::to_value(&answer).expect("serialises");
    assert_eq!(
        written,
        json!({
            "op": "check",
            "requestId": 3,
            "enabled": true,
            "running": true,
            "port": 7420,
            "lastCallSecs": 12,
            "calls": 5
        })
    );
}
