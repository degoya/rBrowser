use serde_json::json;

use super::*;

#[test]
fn an_example_follows_refs_and_cuts_cycles() {
    let root = json!({
        "components": { "schemas": {
            "Node": { "type": "object", "properties": {
                "id": { "type": "string", "format": "uuid", "readOnly": true },
                "label": { "type": "string" },
                "kind": { "enum": ["a", "b"] },
                "size": { "type": ["integer", "null"] },
                "child": { "$ref": "#/components/schemas/Node" },
                "children": { "type": "array", "items": { "$ref": "#/components/schemas/Node" } },
                "other": { "$ref": "other.yaml#/X" },
                "pick": { "oneOf": [{ "type": "boolean" }, { "type": "string" }] }
            } }
        } }
    });
    let start = json!({ "$ref": "#/components/schemas/Node" });
    let mut examples = Examples::new(&root);
    let value = examples.of(&start);
    assert_eq!(
        value,
        json!({ "label": "string", "kind": "a", "size": 0, "children": [], "pick": true })
    );
    assert_eq!(examples.external, ["other.yaml#/X"]);
}

#[test]
fn a_given_example_wins() {
    let root = json!({});
    let first = json!({ "type": "string", "example": "x", "default": "y" });
    let second = json!({ "type": "integer", "default": 5 });
    let third = json!({ "examples": [7] });
    let mut examples = Examples::new(&root);
    assert_eq!(examples.of(&first), json!("x"));
    assert_eq!(examples.of(&second), json!(5));
    assert_eq!(examples.of(&third), json!(7));
}

#[test]
fn a_text_media_type_names_itself() {
    let root = json!({});
    let schema = json!({ "type": "string", "example": "<a/>" });
    let mut examples = Examples::new(&root);
    let mut headers = Vec::new();
    let mut out = Collector::default();
    let body = body(
        &mut examples,
        "application/xml",
        &schema,
        None,
        &mut headers,
        "R",
        &mut out,
    );
    assert_eq!(
        body,
        Body::Raw {
            text: "<a/>".into()
        }
    );
    assert_eq!(headers[0].value, "application/xml");
}

/// The number of JSON nodes in `value`.
fn nodes(value: &Value) -> usize {
    1 + match value {
        Value::Array(items) => items.iter().map(nodes).sum(),
        Value::Object(map) => map.values().map(nodes).sum(),
        _ => 0,
    }
}

#[test]
fn a_fan_out_of_refs_stays_within_the_budget() {
    // Eight properties per level, each a reference to the next level: without a budget the
    // example would hold 8^12 nodes. No path is a cycle, so only the budget stops it.
    let mut schemas = Map::new();
    for level in 0..12 {
        let mut properties = Map::new();
        for fan in 0..8 {
            properties.insert(
                format!("p{fan}"),
                json!({ "$ref": format!("#/components/schemas/L{}", level + 1) }),
            );
        }
        schemas.insert(
            format!("L{level}"),
            json!({ "type": "object", "properties": properties }),
        );
    }
    schemas.insert(
        "L12".into(),
        json!({ "type": "string", "example": "x".repeat(1000) }),
    );
    let root = json!({ "components": { "schemas": schemas } });
    let start = json!({ "$ref": "#/components/schemas/L0" });
    let mut examples = Examples::new(&root);
    let value = examples.of(&start);
    assert!(nodes(&value) <= MAX_NODES, "{} nodes", nodes(&value));
    let text = serde_json::to_string(&value).unwrap_or_default();
    assert!(
        text.len() <= MAX_BYTES + 64 * MAX_NODES,
        "{} bytes",
        text.len()
    );
    // A second example gets a budget of its own.
    let small = json!({ "type": "integer" });
    assert_eq!(examples.of(&small), json!(0));
}

#[test]
fn a_large_given_example_is_cut_by_the_byte_budget() {
    let big = json!({ "example": "y".repeat(MAX_BYTES + 1) });
    let root = json!({});
    let mut examples = Examples::new(&root);
    assert_eq!(examples.of(&big), Value::Null);
}
