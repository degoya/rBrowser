//! Input on iOS (RB-095-04): a phone has no mouse and no wheel. A replayed press and release
//! become a touch of one finger (`pointerType: "touch"`); a move without a press (hover) and a
//! wheel turn have no touch and are left out - the scroll follows through the agent's own
//! command, as on every engine. Keys and text stay keys. Whether Mobile Safari takes every one
//! of these is the spike's question (the job file).

use serde_json::{Value, json};

/// The touch sources of the mouse, wheel and key `sources` (`rb_bidi::input::actions_for`).
pub fn touch_sources(sources: Vec<Value>) -> Vec<Value> {
    sources
        .into_iter()
        .filter_map(|source| match source.get("type").and_then(Value::as_str) {
            Some("wheel") => None,
            Some("pointer") => touch(source),
            _ => Some(source),
        })
        .collect()
}

/// The pointer `source` as a finger; `None` when it only moves.
fn touch(mut source: Value) -> Option<Value> {
    let presses = source
        .get("actions")
        .and_then(Value::as_array)
        .is_some_and(|actions| {
            actions.iter().any(|action| {
                matches!(
                    action.get("type").and_then(Value::as_str),
                    Some("pointerDown" | "pointerUp")
                )
            })
        });
    if !presses {
        return None;
    }
    let object = source.as_object_mut()?;
    object.insert("id".to_owned(), json!("finger"));
    object.insert("parameters".to_owned(), json!({ "pointerType": "touch" }));
    Some(source)
}

#[cfg(test)]
#[path = "touch_tests.rs"]
mod tests;
