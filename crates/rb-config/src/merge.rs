//! Merging `config.json` <- `config.local.json` and recording where each value came from.
//!
//! Objects merge key by key. An array whose items are all objects with a string `id` (both
//! sides) merges item by item on `id`: a matching item is merged, a new id is appended, the
//! order of the base stays. Every other value - other arrays included - is replaced.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::Origin;

/// Merges `overlay` into `base` by the rules of this module.
pub fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(slot) => merge(slot, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (Value::Array(base), Value::Array(overlay))
            if is_id_array(base) && is_id_array(&overlay) =>
        {
            for item in overlay {
                let id = item_id(&item).map(str::to_owned);
                match base
                    .iter_mut()
                    .find(|candidate| item_id(candidate) == id.as_deref())
                {
                    Some(slot) => merge(slot, item),
                    None => base.push(item),
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

/// Where each value of the merged config came from, by dotted key (`sync.scroll`,
/// `breakpoints.mobile.width` - items of an id array by their id). The project file is walked
/// first, the local file overrides. Absent keys are defaults.
pub fn origins(project: &Value, local: Option<&Value>) -> BTreeMap<String, Origin> {
    let mut out = BTreeMap::new();
    record(project, "", Origin::Project, &mut out);
    if let Some(local) = local {
        record(local, "", Origin::Local, &mut out);
    }
    out
}

fn record(value: &Value, prefix: &str, origin: Origin, out: &mut BTreeMap<String, Origin>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (key, child) in map {
                record(child, &join(prefix, key), origin, out);
            }
        }
        Value::Array(items) if !items.is_empty() && is_id_array(items) => {
            for item in items {
                if let Some(id) = item_id(item) {
                    record(item, &join(prefix, id), origin, out);
                }
            }
        }
        _ if prefix.is_empty() => {}
        _ => {
            out.insert(prefix.to_owned(), origin);
        }
    }
}

fn join(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

/// Whether every item is an object with a string `id` (an empty array counts).
fn is_id_array(items: &[Value]) -> bool {
    items.iter().all(|item| item_id(item).is_some())
}

fn item_id(item: &Value) -> Option<&str> {
    item.get("id").and_then(Value::as_str)
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
