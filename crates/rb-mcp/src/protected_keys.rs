//! The keys of the project config marked `x-protected` (RB-010-02: request headers, the network
//! keys, the folders, the breakpoints, extensions, ...), read from `rb_config::project_schema`
//! so the list cannot drift from the annotations, and a config with them left out: what
//! `rbrowser://config` answers (RB-095-01).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde_json::Value;

/// How deep the walk follows properties and references (the schema nests a few levels).
const MAX_DEPTH: usize = 16;
/// The annotation (`rb_settings::X_PROTECTED`; rb-mcp does not depend on rb-settings).
const X_PROTECTED: &str = "x-protected";

/// The dotted paths of every `x-protected` key of the project config, sorted; computed once.
pub fn paths() -> &'static [String] {
    static PATHS: OnceLock<Vec<String>> = OnceLock::new();
    PATHS.get_or_init(|| {
        serde_json::to_value(rb_config::project_schema())
            .map(|schema| protected_paths(&schema))
            .unwrap_or_default()
    })
}

/// The dotted paths of the properties of `schema` (a root schema with `$defs`) that carry
/// `x-protected: true`, sorted. A reference is followed, an `anyOf`/`oneOf`/`allOf` (an
/// optional field) looked into; a cycle ends at the reference seen before.
#[must_use]
pub fn protected_paths(schema: &Value) -> Vec<String> {
    let mut found = BTreeSet::new();
    walk(schema, schema, "", &mut Vec::new(), &mut found);
    found.into_iter().collect()
}

fn walk<'a>(
    root: &'a Value,
    node: &'a Value,
    path: &str,
    refs: &mut Vec<&'a str>,
    found: &mut BTreeSet<String>,
) {
    if refs.len() > MAX_DEPTH || path.split('.').count() > MAX_DEPTH {
        return;
    }
    if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
        if refs.contains(&reference) {
            return;
        }
        if let Some(target) = reference
            .strip_prefix('#')
            .and_then(|pointer| root.pointer(pointer))
        {
            refs.push(reference);
            walk(root, target, path, refs, found);
            refs.pop();
        }
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        for part in node
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            walk(root, part, path, refs, found);
        }
    }
    let Some(properties) = node.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (name, property) in properties {
        let child = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}.{name}")
        };
        if property.get(X_PROTECTED) == Some(&Value::Bool(true)) {
            found.insert(child);
            continue;
        }
        walk(root, property, &child, refs, found);
    }
}

/// `config` without the keys at `paths` (dotted; a path whose parent is no object is skipped).
#[must_use]
pub fn without(mut config: Value, paths: &[String]) -> Value {
    for path in paths {
        let (parent, last) = match path.rsplit_once('.') {
            Some((parent, last)) => (Some(parent), last),
            None => (None, path.as_str()),
        };
        let holder = match parent {
            None => Some(&mut config),
            Some(parent) => parent
                .split('.')
                .try_fold(&mut config, |node, segment| node.get_mut(segment)),
        };
        if let Some(Value::Object(map)) = holder {
            map.remove(last);
        }
    }
    config
}

#[cfg(test)]
#[path = "protected_keys_tests.rs"]
mod tests;
