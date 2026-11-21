//! An OpenAPI or Swagger schema's example value, and an operation's body from its media types.
//! An `example`, `default`, `const` or the first `enum` value wins; else a value of the type
//! (`string` by its format, 0, `true`, an object of its properties without the read-only ones,
//! an array of one item); `allOf` merged, `oneOf`/`anyOf` the first; a `$ref` followed inside the
//! document, a cycle cut, a reference to another file noted for the report. One example holds at
//! most `MAX_NODES` nodes and `MAX_BYTES` bytes of text: a fan-out of references without a cycle
//! would else grow it exponentially; past the budget a node is `null`.

use rb_api_model::{Body, Entry, Part};
use serde_json::{Map, Value};

use crate::collect::Collector;
use crate::common;

/// How deep an example is built.
const MAX_DEPTH: usize = 12;
/// How many `$ref` hops are followed for one node.
const MAX_HOPS: usize = 16;
/// How many nodes one example holds at most.
const MAX_NODES: usize = 10_000;
/// How many bytes of text (strings and keys of given examples, generated strings) one example
/// holds at most.
const MAX_BYTES: usize = 1 << 20;

/// Builds examples over one document.
pub struct Examples<'a> {
    root: &'a Value,
    seen: Vec<String>,
    /// The nodes the current example may still hold.
    nodes_left: usize,
    /// The bytes of text the current example may still hold.
    bytes_left: usize,
    /// The references to other files met, each once.
    pub external: Vec<String>,
}

impl<'a> Examples<'a> {
    #[must_use]
    pub fn new(root: &'a Value) -> Self {
        Self {
            root,
            seen: Vec::new(),
            nodes_left: MAX_NODES,
            bytes_left: MAX_BYTES,
            external: Vec::new(),
        }
    }

    /// `value` with its local `$ref`s followed; `None` for a reference to another file or one
    /// that leads nowhere.
    pub fn resolve(&mut self, value: &'a Value) -> Option<&'a Value> {
        let mut current = value;
        for _ in 0..MAX_HOPS {
            let Some(reference) = current["$ref"].as_str() else {
                return Some(current);
            };
            match reference.strip_prefix('#') {
                Some(pointer) => current = self.root.pointer(pointer)?,
                None => {
                    if !self.external.iter().any(|known| known == reference) {
                        self.external.push(reference.to_owned());
                    }
                    return None;
                }
            }
        }
        None
    }

    /// The example of `schema`.
    pub fn of(&mut self, schema: &'a Value) -> Value {
        self.nodes_left = MAX_NODES;
        self.bytes_left = MAX_BYTES;
        self.at(schema, 0)
    }

    fn at(&mut self, schema: &'a Value, depth: usize) -> Value {
        if depth > MAX_DEPTH || self.nodes_left == 0 {
            return Value::Null;
        }
        self.nodes_left -= 1;
        let reference = schema["$ref"].as_str().map(str::to_owned);
        if let Some(reference) = &reference {
            if self.seen.contains(reference) {
                return Value::Null;
            }
            self.seen.push(reference.clone());
        }
        let value = match self.resolve(schema) {
            Some(schema) => self.own(schema, depth),
            None => Value::Null,
        };
        if reference.is_some() {
            self.seen.pop();
        }
        value
    }

    fn own(&mut self, schema: &'a Value, depth: usize) -> Value {
        for key in ["example", "default", "const"] {
            if let Some(value) = schema.get(key) {
                return self.given(value);
            }
        }
        if let Some(first) = schema["examples"]
            .as_array()
            .and_then(|items| items.first())
        {
            return self.given(first);
        }
        if let Some(first) = schema["enum"].as_array().and_then(|items| items.first()) {
            return self.given(first);
        }
        if let Some(parts) = schema["allOf"].as_array() {
            let mut merged = Map::new();
            for part in parts {
                if let Value::Object(map) = self.at(part, depth + 1) {
                    merged.extend(map);
                }
            }
            return Value::Object(merged);
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(first) = schema[key].as_array().and_then(|items| items.first()) {
                return self.at(first, depth + 1);
            }
        }
        let kind = match &schema["type"] {
            Value::String(kind) => kind.as_str(),
            Value::Array(kinds) => kinds
                .iter()
                .filter_map(Value::as_str)
                .find(|kind| *kind != "null")
                .unwrap_or("null"),
            _ if schema["properties"].is_object() => "object",
            _ if schema["items"].is_object() => "array",
            _ => "",
        };
        match kind {
            "object" => {
                let mut out = Map::new();
                if let Value::Object(properties) = &schema["properties"] {
                    for (name, property) in properties {
                        let read_only = self
                            .resolve(property)
                            .is_some_and(|p| p["readOnly"] == Value::Bool(true));
                        if read_only {
                            continue;
                        }
                        let value = self.at(property, depth + 1);
                        if !value.is_null() {
                            out.insert(name.clone(), value);
                        }
                    }
                }
                Value::Object(out)
            }
            "array" => {
                let item = self.at(&schema["items"], depth + 1);
                Value::Array(if item.is_null() {
                    Vec::new()
                } else {
                    vec![item]
                })
            }
            "string" => {
                let text = string_of(schema["format"].as_str().unwrap_or(""));
                if !charge(text.len(), &mut self.bytes_left) {
                    return Value::Null;
                }
                Value::String(text.to_owned())
            }
            "integer" | "number" => Value::from(0),
            "boolean" => Value::Bool(true),
            _ => Value::Null,
        }
    }

    /// A copy of a value the document gives, charged to the budget; `null` when it does not fit.
    fn given(&mut self, value: &Value) -> Value {
        // The node `at` charged for this schema is the value's own root.
        let mut nodes = self.nodes_left.saturating_add(1);
        let mut bytes = self.bytes_left;
        if !weigh(value, &mut nodes, &mut bytes) {
            return Value::Null;
        }
        self.nodes_left = nodes;
        self.bytes_left = bytes;
        value.clone()
    }
}

/// Charges `value`'s nodes and text bytes to `nodes` and `bytes`; false, stopping early, when it
/// does not fit.
fn weigh(value: &Value, nodes: &mut usize, bytes: &mut usize) -> bool {
    if *nodes == 0 {
        return false;
    }
    *nodes -= 1;
    match value {
        Value::String(text) => charge(text.len(), bytes),
        Value::Array(items) => items.iter().all(|item| weigh(item, nodes, bytes)),
        Value::Object(map) => map
            .iter()
            .all(|(key, item)| charge(key.len(), bytes) && weigh(item, nodes, bytes)),
        _ => true,
    }
}

/// Takes `len` from `bytes`; false when fewer are left.
fn charge(len: usize, bytes: &mut usize) -> bool {
    if len > *bytes {
        return false;
    }
    *bytes -= len;
    true
}

fn string_of(format: &str) -> &'static str {
    match format {
        "date-time" => "2024-01-01T00:00:00Z",
        "date" => "2024-01-01",
        "time" => "00:00:00",
        "email" => "user@example.com",
        "uuid" => "00000000-0000-4000-8000-000000000000",
        "uri" | "url" => "https://example.com",
        "hostname" => "example.com",
        "ipv4" => "192.0.2.1",
        "ipv6" => "2001:db8::1",
        "byte" | "binary" | "password" => "",
        _ => "string",
    }
}

/// An example as a form value: a string as it is, else its JSON text.
#[must_use]
pub fn as_field(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Whether a property of `schema` is a file (`format: binary`, Swagger's `type: file`).
fn is_file(schema: &Value) -> bool {
    schema["format"].as_str() == Some("binary") || schema["type"].as_str() == Some("file")
}

/// A body of media type `media` with `schema` (and the media's own `example`), a header added
/// where the body's kind does not say the type.
pub fn body<'a>(
    examples: &mut Examples<'a>,
    media: &str,
    schema: &'a Value,
    example: Option<&'a Value>,
    headers: &mut Vec<Entry>,
    at: &str,
    out: &mut Collector,
) -> Body {
    let media = media.to_ascii_lowercase();
    if media == "application/x-www-form-urlencoded" || media == "multipart/form-data" {
        let resolved = examples.resolve(schema);
        let mut fields = Vec::new();
        if let Some(Value::Object(properties)) = resolved.map(|s| &s["properties"]) {
            for (name, property) in properties {
                let file = examples.resolve(property).is_some_and(is_file);
                let example = examples.of(property);
                fields.push((name.clone(), as_field(&example), file));
            }
        }
        return if media == "multipart/form-data" {
            Body::Multipart {
                parts: fields
                    .into_iter()
                    .map(|(name, value, file)| Part {
                        file: file.then(|| common::local_file(&name, at, out)),
                        value: if file { String::new() } else { value },
                        name,
                        content_type: None,
                    })
                    .collect(),
            }
        } else {
            Body::Form {
                fields: fields
                    .into_iter()
                    .map(|(name, value, _)| Entry::new(name, value))
                    .collect(),
            }
        };
    }
    let example = example.cloned().unwrap_or_else(|| examples.of(schema));
    if media == "application/json" || media.ends_with("+json") || media.contains("/json") {
        if media != "application/json" && common::content_type(headers).is_empty() {
            headers.push(Entry::new("Content-Type", media.clone()));
        }
        let text = serde_json::to_string_pretty(&example).unwrap_or_default();
        return Body::Json { text };
    }
    if media == "application/octet-stream" || media.starts_with("image/") {
        if common::content_type(headers).is_empty() {
            headers.push(Entry::new("Content-Type", media.clone()));
        }
        return Body::Binary {
            path: common::local_file("body", at, out),
        };
    }
    if common::content_type(headers).is_empty() && !media.is_empty() {
        headers.push(Entry::new("Content-Type", media.clone()));
    }
    let text = match example {
        Value::String(text) => text,
        Value::Null => String::new(),
        other => other.to_string(),
    };
    Body::Raw { text }
}

#[cfg(test)]
#[path = "openapi_example_tests.rs"]
mod tests;
