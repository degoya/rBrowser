use serde_json::json;

use super::{McpClient, McpScriptSettings, McpSettings};

#[test]
fn prompts_and_resources_are_on_by_default() {
    let defaults = McpSettings::default();
    assert!(defaults.prompts);
    assert!(defaults.resources);
    // A settings file from before RB-095-01 has neither key: both on.
    let older: McpSettings =
        serde_json::from_value(json!({ "enabled": true })).expect("an older mcp section");
    assert!(older.prompts && older.resources && older.enabled);
}

#[test]
fn prompts_and_resources_switch_off_one_by_one() {
    let settings: McpSettings =
        serde_json::from_value(json!({ "prompts": false })).expect("prompts off");
    assert!(!settings.prompts);
    assert!(settings.resources);
    let written = serde_json::to_value(McpSettings {
        resources: false,
        ..McpSettings::default()
    })
    .expect("serialises");
    assert_eq!(written["resources"], false);
    assert_eq!(written["prompts"], true);
}

#[test]
fn the_setup_client_is_claude_code_by_default_and_named_by_its_id() {
    assert_eq!(McpSettings::default().client, McpClient::ClaudeCode);
    for client in McpClient::ALL {
        let written = serde_json::to_value(client).expect("serialises");
        assert_eq!(written, client.id());
        let read: McpClient = serde_json::from_value(written).expect("reads back");
        assert_eq!(read, client);
    }
    let settings: McpSettings =
        serde_json::from_value(json!({ "client": "codex" })).expect("a chosen client");
    assert_eq!(settings.client, McpClient::Codex);
    assert!(serde_json::from_value::<McpClient>(json!("emacs")).is_err());
}

#[test]
fn network_is_collected_on_demand_by_default() {
    // RB-090-34: off by default, the first read of the requests switches it on.
    assert!(!McpSettings::default().collect_network);
    let settings: McpSettings =
        serde_json::from_value(json!({ "collectNetwork": true })).expect("collect network");
    assert!(settings.collect_network);
}

#[test]
fn code_mode_is_off_by_default_with_bounded_runs() {
    // RB-098-01: a person switches `script_run` on; the limits have defaults.
    let defaults = McpSettings::default().scripts;
    assert_eq!(
        defaults,
        McpScriptSettings {
            enabled: false,
            timeout_ms: 30_000,
            max_calls: 200,
            max_result_bytes: 65_536,
        }
    );
    let settings: McpSettings =
        serde_json::from_value(json!({ "scripts": { "enabled": true, "maxCalls": 20 } }))
            .expect("a scripts section");
    assert!(settings.scripts.enabled);
    assert_eq!(settings.scripts.max_calls, 20);
    assert_eq!(settings.scripts.timeout_ms, 30_000);
    let written = serde_json::to_value(McpSettings::default()).expect("serialises");
    assert_eq!(written["scripts"]["maxResultBytes"], 65_536);
    assert_eq!(written["scripts"]["timeoutMs"], 30_000);
}

#[test]
fn a_script_ends_before_the_stdio_forward_gives_up() {
    // `rbrowser mcp` waits 600 s for an answer (rb-mcp `stdio.rs`, REQUEST_TIMEOUT); the longest
    // script plus the server's 2 s of grace stays below it.
    let schema = serde_json::to_value(schemars::schema_for!(McpScriptSettings)).expect("schema");
    let maximum = schema
        .pointer("/properties/timeoutMs/maximum")
        .and_then(serde_json::Value::as_u64)
        .expect("a maximum");
    assert_eq!(maximum, 590_000);
    assert!(maximum + 2_000 < 600_000);
}
