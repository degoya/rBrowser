use super::*;

#[test]
fn the_anthropic_format_has_every_core_tool_as_a_custom_tool() {
    let tools = definitions(ToolFormat::Anthropic, None);
    let tools = tools.as_array().expect("a list");
    assert_eq!(tools.len(), RbMcpServer::tool_names().len());
    for tool in tools {
        assert!(tool["name"].is_string());
        assert!(
            tool["description"]
                .as_str()
                .is_some_and(|text| text.len() > 20)
        );
        assert_eq!(tool["input_schema"]["type"], "object", "{}", tool["name"]);
        assert!(tool.get("inputSchema").is_none());
        assert!(tool.get("allowed_callers").is_none());
    }
    let script = tools
        .iter()
        .find(|tool| tool["name"] == crate::scripts::TOOL)
        .expect("script_run");
    assert!(
        script["description"]
            .as_str()
            .is_some_and(|text| text.contains("rb.navigate({"))
    );
}

#[test]
fn an_allowed_caller_is_named_on_every_tool() {
    let tools = definitions(ToolFormat::Anthropic, Some("code_execution_20260120"));
    for tool in tools.as_array().expect("a list") {
        assert_eq!(tool["allowed_callers"], json!(["code_execution_20260120"]));
    }
}

#[test]
fn the_mcp_format_is_the_tool_list() {
    let tools = definitions(ToolFormat::Mcp, Some("ignored"));
    let first = &tools[0];
    assert_eq!(first["inputSchema"]["type"], "object");
    assert!(first.get("input_schema").is_none());
    assert!(first.get("allowed_callers").is_none());
}
