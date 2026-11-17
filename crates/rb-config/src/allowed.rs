//! Values outside the choice a schema offers (RB-080-72): a string with an `enum` - on a key or
//! on the items of a list - whose type takes any text (`#[schemars(extend("enum" = ...))]` on a
//! `String`, the Lighthouse categories). The config keeps such a value, so the settings window
//! shows it and offers to remove it; each one is a warning with a sentence. Derived from the
//! schema, so a new enum of that kind is covered without a line here.

use rb_util::json_pointer::escape;
use serde_json::Value;

use crate::ConfigIssue;

/// The i18n key of a value outside the values a key allows, which the config keeps.
pub const NOT_ALLOWED_KEY: &str = "config.warning.not_allowed";

/// Every string of `value` that its place in `schema` does not allow, as a warning at its JSON
/// pointer.
pub fn not_allowed(schema: &Value, value: &Value) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    walk(schema, value, "", &mut issues);
    issues
}

fn walk(schema: &Value, value: &Value, at: &str, issues: &mut Vec<ConfigIssue>) {
    // `Option<T>` is an `anyOf` of `T` and `null`: the branches that are no `null` count.
    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
        for branch in branches.iter().filter(|branch| branch["type"] != "null") {
            walk(branch, value, at, issues);
        }
    }
    match value {
        Value::String(text) => {
            if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
                && !allowed.iter().any(|choice| choice.as_str() == Some(text))
            {
                let names: Vec<&str> = allowed.iter().filter_map(Value::as_str).collect();
                issues.push(ConfigIssue {
                    path: at.to_owned(),
                    key: NOT_ALLOWED_KEY.to_owned(),
                    message: format!(
                        "`{text}` is not one of the allowed values ({}); it is ignored",
                        names.join(", ")
                    ),
                });
            }
        }
        Value::Array(items) => {
            if let Some(item_schema) = schema.get("items") {
                for (index, item) in items.iter().enumerate() {
                    walk(item_schema, item, &format!("{at}/{index}"), issues);
                }
            }
        }
        Value::Object(fields) => {
            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                for (name, field) in fields {
                    if let Some(field_schema) = properties.get(name) {
                        walk(
                            field_schema,
                            field,
                            &format!("{at}/{}", escape(name)),
                            issues,
                        );
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "allowed_tests.rs"]
mod tests;
