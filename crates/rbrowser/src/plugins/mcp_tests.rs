use rb_plugin_host::PluginTool;
use serde_json::json;

use super::allowed;

fn tool(evaluates_input: bool) -> PluginTool {
    PluginTool {
        name: "sharedconsole_exec".to_owned(),
        plugin_id: "shared-console".to_owned(),
        tool: "exec".to_owned(),
        description: "plugin.shared-console.tools.exec".to_owned(),
        input_schema: json!({ "type": "object" }),
        evaluates_input,
    }
}

#[test]
fn a_tool_running_its_callers_script_needs_allow_evaluate() {
    let refused = allowed(&tool(true), false).expect_err("evaluate is off");
    assert_eq!(refused.code, rb_mcp::tools::page::EVALUATE_DISABLED);
    assert!(allowed(&tool(true), true).is_ok());
    assert!(allowed(&tool(false), false).is_ok());
}

#[test]
fn the_api_clients_panel_tool_is_never_an_mcp_tool() {
    let mut panel = tool(false);
    panel.plugin_id = "api-client".to_owned();
    panel.tool = "panel".to_owned();
    assert!(super::panel_only(&panel));
    panel.tool = "run".to_owned();
    assert!(!super::panel_only(&panel));
    assert!(!super::panel_only(&tool(false)));
}
