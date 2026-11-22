use serde_json::json;

use super::*;

fn tool(name: &str, schema: Value) -> Tool {
    let Value::Object(schema) = schema else {
        panic!("an object schema");
    };
    Tool::new(name.to_owned(), "Does a thing to the views.", schema)
}

#[test]
fn each_tool_is_one_line_with_its_required_and_optional_arguments() {
    let tools = [
        tool(
            "navigate",
            json!({ "type": "object",
                    "properties": { "url": {}, "target": {}, "waitUntil": {} },
                    "required": ["url"] }),
        ),
        tool("views_list", json!({ "type": "object" })),
        tool(
            TOOL,
            json!({ "type": "object", "properties": { "code": {} } }),
        ),
    ];
    assert_eq!(
        signatures(&tools),
        "rb.navigate({ url, target?, waitUntil? })\nrb.views_list({})"
    );
}

#[test]
fn the_description_keeps_its_text_and_adds_the_signatures() {
    let text = description(
        "Run a script.",
        &[tool("reload", json!({ "type": "object" }))],
    );
    assert!(text.starts_with("Run a script.\n\n"), "{text}");
    assert!(text.ends_with("rb.reload({})"), "{text}");
}

#[test]
fn the_servers_script_run_lists_every_core_and_plugin_tool() {
    let plugins = crate::tools::plugins::router(vec![crate::PluginToolInfo {
        name: "hello_greet".to_owned(),
        description: "Says hello.".to_owned(),
        input_schema: json!({ "type": "object", "properties": { "name": {} } }),
    }]);
    let mut router = RbMcpServer::router() + plugins;
    describe(&mut router);
    let text = router
        .get(TOOL)
        .and_then(|tool| tool.description.clone())
        .expect("script_run is served");
    assert!(text.contains("rb.navigate({"), "{text}");
    assert!(
        text.contains("rb.hello_greet({ name?, target? })"),
        "{text}"
    );
    assert!(!text.contains("rb.script_run("), "{text}");
}
