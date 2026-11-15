//! The entities of a page's JSON-LD blocks, so an item of its Microdata or RDFa can be held to
//! the JSON-LD of the same entity: every object with an `@type` (in `@graph`, arrays and nested
//! objects too), its schema.org types by short name and its scalar properties as text.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::page::JsonLd;

/// The most entities read from one page.
pub const MAX_ENTITIES: usize = 500;
/// The deepest nesting followed.
const MAX_DEPTH: usize = 16;

/// One JSON-LD entity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entity {
    pub types: Vec<String>,
    /// Scalar properties (strings, numbers, booleans) as text; `@id` among them.
    pub props: BTreeMap<String, String>,
}

/// A schema.org type name without its IRI.
#[must_use]
pub fn short_type(name: &str) -> String {
    let name = name.trim();
    for prefix in ["https://schema.org/", "http://schema.org/", "schema:"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            return rest.to_owned();
        }
    }
    name.to_owned()
}

fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.split_whitespace().collect::<Vec<_>>().join(" ")),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn walk(value: &Value, depth: usize, out: &mut Vec<Entity>) {
    if depth > MAX_DEPTH || out.len() >= MAX_ENTITIES {
        return;
    }
    match value {
        Value::Array(values) => {
            for value in values {
                walk(value, depth + 1, out);
            }
        }
        Value::Object(map) => {
            let types: Vec<String> = match map.get("@type") {
                Some(Value::String(name)) => vec![short_type(name)],
                Some(Value::Array(names)) => names
                    .iter()
                    .filter_map(Value::as_str)
                    .map(short_type)
                    .collect(),
                _ => Vec::new(),
            };
            if !types.is_empty() {
                let props = map
                    .iter()
                    .filter(|(key, _)| *key != "@type" && *key != "@context")
                    .filter_map(|(key, value)| Some((key.clone(), scalar(value)?)))
                    .collect();
                out.push(Entity { types, props });
            }
            for (key, value) in map {
                if key != "@context" {
                    walk(value, depth + 1, out);
                }
            }
        }
        _ => {}
    }
}

/// Every entity of `blocks`; a block that is no JSON is skipped (`structured.jsonld_invalid`
/// names it).
#[must_use]
pub fn entities(blocks: &[JsonLd]) -> Vec<Entity> {
    let mut out = Vec::new();
    for block in blocks {
        if let Ok(value) = serde_json::from_str::<Value>(&block.text) {
            walk(&value, 0, &mut out);
        }
    }
    out
}

#[cfg(test)]
#[path = "jsonld_tests.rs"]
mod tests;
