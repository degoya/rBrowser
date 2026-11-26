use std::sync::Arc;

use rmcp::model::{JsonObject, Tool};
use serde_json::{Value, json};

use super::{
    TARGET_ACTIVE, TARGET_ALL, TargetSupport, areas, definition_problems, target_support,
    target_text_problem, tool_problems,
};
use crate::RbMcpServer;

fn schema(value: Value) -> Arc<JsonObject> {
    match value {
        Value::Object(map) => Arc::new(map),
        other => panic!("not an object: {other}"),
    }
}

fn tool(name: &'static str, input: Value) -> Tool {
    Tool::new(
        name,
        "A tool of the test, described well enough.",
        schema(input),
    )
}

/// `target` as schemars writes `Option<TargetArg>`: a reference to the untagged enum.
fn targeted(description: &str) -> Value {
    json!({
        "type": "object",
        "properties": {
            "target": {
                "description": description,
                "anyOf": [{ "$ref": "#/$defs/TargetArg" }, { "type": "null" }],
            },
        },
        "$defs": {
            "TargetArg": {
                "anyOf": [
                    { "type": "integer", "format": "uint32", "minimum": 0 },
                    { "type": "string", "description": "`\"all\"`, a view id as text" },
                ],
            },
        },
    })
}

#[test]
fn the_registered_tools_break_no_rule() {
    assert_eq!(tool_problems(), Vec::<String>::new());
}

#[test]
fn the_areas_hold_every_served_tool_once() {
    let mut names: Vec<String> = areas()
        .into_iter()
        .flat_map(|(_, tools)| tools.into_iter().map(|tool| tool.name.into_owned()))
        .collect();
    names.sort();
    assert_eq!(names, RbMcpServer::tool_names());
}

#[test]
fn a_view_tool_takes_all_or_a_view_id() {
    let navigate = tool("navigate", targeted("`\"all\"` (default), a view id."));
    assert_eq!(target_support(&navigate), Ok(TargetSupport::All));
    let devtools = tool(
        "devtools_open",
        targeted("One view; the active view when absent."),
    );
    assert_eq!(target_support(&devtools), Ok(TargetSupport::One));
    let settings = tool(
        "settings_get",
        json!({ "type": "object", "properties": {} }),
    );
    assert_eq!(target_support(&settings), Ok(TargetSupport::None));
}

#[test]
fn a_view_tool_without_target_or_without_all_is_refused() {
    let bare = tool("teleport", json!({ "type": "object", "properties": {} }));
    let found = target_support(&bare).expect_err("no target");
    assert!(found.contains("UNTARGETED"), "{found}");
    let one = tool("teleport", targeted("One view."));
    let found = target_support(&one).expect_err("no all");
    assert!(found.contains("SINGLE_VIEW"), "{found}");
    let listed = tool("settings_get", targeted("`\"all\"`."));
    let found = target_support(&listed).expect_err("listed untargeted");
    assert!(found.contains("listed in UNTARGETED"), "{found}");
}

#[test]
fn a_target_that_takes_only_text_is_refused() {
    let text_only = tool(
        "navigate",
        json!({
            "type": "object",
            "properties": { "target": { "type": "string", "description": "`\"all\"`." } },
        }),
    );
    let found = target_support(&text_only).expect_err("text only");
    assert!(found.contains("TargetArg"), "{found}");
}

#[test]
fn a_tool_without_description_or_object_schema_is_refused() {
    let mut bare = Tool::new_with_raw("settings_get", None, schema(json!({ "type": "string" })));
    let found = definition_problems(&bare);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].starts_with("no description"), "{found:?}");
    assert!(found[1].contains("object schema"), "{found:?}");
    bare.description = Some("short".into());
    assert!(definition_problems(&bare)[0].starts_with("no description"));
}

#[test]
fn a_tool_that_takes_unknown_arguments_is_refused() {
    let open = tool(
        "settings_get",
        json!({ "type": "object", "properties": {} }),
    );
    let found = definition_problems(&open);
    assert!(
        found.iter().any(|why| why.contains("additionalProperties")),
        "{found:?}"
    );
    let closed = tool(
        "settings_get",
        json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    );
    assert_eq!(definition_problems(&closed), Vec::<String>::new());
}

#[test]
fn a_tool_without_arguments_refuses_any() {
    let schema = rmcp::handler::server::common::schema_for_type::<crate::tools::NoArgs>();
    assert_eq!(schema.get("type"), Some(&json!("object")));
    assert_eq!(schema.get("additionalProperties"), Some(&json!(false)));
    let unknown: Result<crate::tools::NoArgs, _> = serde_json::from_value(json!({ "x": 1 }));
    assert!(unknown.is_err());
    let empty: Result<crate::tools::NoArgs, _> = serde_json::from_value(json!({}));
    assert!(empty.is_ok());
}

#[test]
fn the_target_text_follows_the_default_rule() {
    let navigate = tool("navigate", targeted(TARGET_ALL));
    assert_eq!(target_text_problem(&navigate), None);
    let wrapped = tool(
        "click",
        targeted("`\"all\"`, a view id or a breakpoint id; the active view\nwhen absent."),
    );
    assert_eq!(target_text_problem(&wrapped), None);
    let click = tool("click", targeted(TARGET_ALL));
    let found = target_text_problem(&click).expect("click defaults to the active view");
    assert!(found.contains("ACTIVE_DEFAULT"), "{found}");
    let reload = tool("reload", targeted(TARGET_ACTIVE));
    assert!(target_text_problem(&reload).is_some());
    let devtools = tool(
        "devtools_open",
        targeted("One view: a view id or a breakpoint id shown once; the active view when absent."),
    );
    assert_eq!(target_text_problem(&devtools), None);
    let untargeted = tool(
        "settings_get",
        json!({ "type": "object", "properties": {} }),
    );
    assert_eq!(target_text_problem(&untargeted), None);
}
