use serde_json::json;

use super::{McpClient, McpSettings};

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
