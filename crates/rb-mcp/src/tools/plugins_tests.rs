use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::testing::FakeBackend;

fn info(name: &str, schema: Value) -> PluginToolInfo {
    PluginToolInfo {
        name: name.to_owned(),
        description: "Says hello.".to_owned(),
        input_schema: schema,
    }
}

#[test]
fn every_plugin_tool_takes_a_target_beside_its_own_arguments() {
    let schema = with_target(&json!({
        "type": "object",
        "properties": { "name": { "type": "string" } },
        "required": ["name"],
    }));
    assert_eq!(schema["required"], json!(["name"]));
    assert_eq!(schema["properties"]["name"], json!({ "type": "string" }));
    assert!(schema["properties"]["target"]["description"].is_string());
    let bare = with_target(&json!("no schema"));
    assert_eq!(bare["type"], "object");
    assert!(bare["properties"]["target"].is_object());
}

#[test]
fn plugin_tools_are_listed_with_their_description_and_never_shadow_a_core_tool() {
    let tools = router(vec![
        info("hello_greet", json!({ "type": "object" })),
        info("views_list", json!({ "type": "object" })),
    ]);
    let listed: Vec<String> = tools
        .list_all()
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect();
    assert_eq!(listed, ["hello_greet"]);
    let greet = tools.get("hello_greet").expect("listed");
    assert_eq!(greet.description.as_deref(), Some("Says hello."));
    assert!(greet.input_schema.contains_key("properties"));
}

#[test]
fn the_server_offers_the_backends_plugin_tools() {
    let backend = Arc::new(FakeBackend::default());
    backend.set_plugin_tools(vec![info("hello_greet", json!({ "type": "object" }))]);
    let server = RbMcpServer::new(backend);
    assert!(server.tool_router.has_route("hello_greet"));
    assert!(server.tool_router.has_route("views_list"));
}

#[test]
fn a_target_is_all_a_view_id_a_breakpoint_id_or_a_list() {
    assert_eq!(target_of(None).expect("none"), None);
    assert_eq!(
        target_of(Some(json!(3))).expect("an id"),
        Some(RunTarget::Id(3))
    );
    assert_eq!(
        target_of(Some(json!("mobile"))).expect("a name"),
        Some(RunTarget::Name("mobile".to_owned()))
    );
    assert_eq!(
        target_of(Some(json!(["mobile", 2]))).expect("a list"),
        Some(RunTarget::List(vec![
            crate::run_target::TargetItem::Name("mobile".to_owned()),
            crate::run_target::TargetItem::Id(2),
        ]))
    );
    assert!(target_of(Some(json!({ "x": 1 }))).is_err());
}

/// RB-099-10: a tool that takes `breakpoints` says its target takes "visible" and a list.
#[test]
fn a_tool_taking_breakpoints_takes_visible_and_a_list_as_its_target() {
    let scoped = json!({
        "type": "object",
        "properties": { "breakpoints": { "type": "array", "items": { "type": "string" } } },
    });
    assert!(takes_breakpoints(&scoped));
    let schema = with_target(&scoped);
    let target = &schema["properties"]["target"];
    assert!(
        target["description"]
            .as_str()
            .is_some_and(|text| text.contains("\"visible\"") && text.contains("\"all\""))
    );
    assert_eq!(target["anyOf"][2]["type"], "array");
    let plain = with_target(&json!({ "type": "object" }));
    assert!(!takes_breakpoints(&json!({ "type": "object" })));
    assert_eq!(
        plain["properties"]["target"]["anyOf"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
}
