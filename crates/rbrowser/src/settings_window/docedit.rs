//! Editing a settings file's JSON in place: a write changes only what differs from the value in
//! effect, keeps the order of every key it does not touch (serde_json's `preserve_order`), and
//! a reset removes the key and the objects it leaves empty. So saving one field changes one key
//! of `config.json` (RB-010-08, the diff test).

use serde_json::{Map, Value};

/// The value at the dotted `key` of `document`.
pub fn lookup<'a>(document: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.')
        .try_fold(document, |node, segment| node.as_object()?.get(segment))
}

/// Writes `new` at `key` of `document` (a settings file), where `old` is the value in effect:
/// only what differs from `old` reaches the file. Parent objects are created as needed; a
/// parent that is not an object is an error.
pub fn set(document: &mut Value, key: &str, old: &Value, new: &Value) -> Result<(), String> {
    let segments: Vec<&str> = key.split('.').collect();
    let Some((last, parents)) = segments.split_last() else {
        return Err("empty key".to_owned());
    };
    if !document.is_object() {
        *document = Value::Object(Map::new());
    }
    let mut node = document;
    for segment in parents {
        let object = node
            .as_object_mut()
            .ok_or_else(|| format!("`{segment}` sits in a value that is not an object"))?;
        node = object
            .entry((*segment).to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    let object = node
        .as_object_mut()
        .ok_or_else(|| format!("the parent of `{last}` is not an object"))?;
    match object.get_mut(*last) {
        // A plain value asked for is written as asked, whatever is in effect (another file
        // may override this one); a list or object changes only where it differs.
        Some(slot) if !new.is_object() && !new.is_array() => *slot = new.clone(),
        Some(slot) => update(slot, old, new),
        None => {
            object.insert((*last).to_owned(), changes(old, new));
        }
    }
    Ok(())
}

/// Removes `key` from `document` and every parent object the removal leaves empty.
pub fn remove(document: &mut Value, key: &str) {
    let segments: Vec<&str> = key.split('.').collect();
    remove_at(document, &segments);
}

fn remove_at(node: &mut Value, segments: &[&str]) {
    let Some((first, rest)) = segments.split_first() else {
        return;
    };
    let Some(object) = node.as_object_mut() else {
        return;
    };
    if rest.is_empty() {
        object.shift_remove(*first);
        return;
    }
    if let Some(child) = object.get_mut(*first) {
        remove_at(child, rest);
        if child.as_object().is_some_and(Map::is_empty) {
            object.shift_remove(*first);
        }
    }
}

/// Updates `slot` (what the file holds) from `old` (in effect) to `new`: objects key by key,
/// arrays of the same length item by item, anything else replaced.
fn update(slot: &mut Value, old: &Value, new: &Value) {
    if old == new && !slot.is_null() {
        return;
    }
    match (slot, old, new) {
        (Value::Object(file), Value::Object(old), Value::Object(new)) => {
            let gone: Vec<String> = old
                .keys()
                .filter(|key| !new.contains_key(key.as_str()))
                .cloned()
                .collect();
            for key in gone {
                file.shift_remove(&key);
            }
            for (key, value) in new {
                let before = old.get(key).unwrap_or(&Value::Null);
                match file.get_mut(key) {
                    Some(child) => update(child, before, value),
                    None if value == before => {}
                    None => {
                        file.insert(key.clone(), changes(before, value));
                    }
                }
            }
        }
        (Value::Array(file), Value::Array(old), Value::Array(new))
            if file.len() == new.len() && old.len() == new.len() =>
        {
            for ((item, before), value) in file.iter_mut().zip(old).zip(new) {
                update(item, before, value);
            }
        }
        (slot, _, new) => *slot = new.clone(),
    }
}

/// What a file that does not hold the key yet gets: of an object only the fields that differ
/// from `old`, anything else whole.
fn changes(old: &Value, new: &Value) -> Value {
    match (old, new) {
        (Value::Object(old), Value::Object(new)) => Value::Object(
            new.iter()
                .filter(|(key, value)| old.get(key.as_str()) != Some(*value))
                .map(|(key, value)| {
                    let before = old.get(key).unwrap_or(&Value::Null);
                    (key.clone(), changes(before, value))
                })
                .collect(),
        ),
        _ => new.clone(),
    }
}

#[cfg(test)]
#[path = "docedit_tests.rs"]
mod tests;
