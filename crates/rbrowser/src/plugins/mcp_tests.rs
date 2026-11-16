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
