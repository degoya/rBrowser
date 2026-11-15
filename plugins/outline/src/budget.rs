//! An answer cut to fit a byte budget: a panel's answer may hold at most 64 KiB of JSON
//! (`docs/plugins.md`, "Limits at a glance"), so the panel asks `get` for one section with a
//! budget, and the longest lists - rows, tree nodes, findings - are halved until it fits. A cut
//! list's object gets `"cut": true`; nothing is cut that fits.

use serde_json::Value;

/// The JSON pointers of the lists that may be cut, with their lengths.
fn lists(answer: &Value) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut push = |pointer: String, value: &Value| {
        if let Some(list) = value.as_array() {
            found.push((pointer, list.len()));
        }
    };
    push("/findings".to_owned(), &answer["findings"]);
    if let Some(sections) = answer["sections"].as_object() {
        for (name, section) in sections {
            push(format!("/sections/{name}/rows"), &section["rows"]);
            if let Some(selectors) = section["selectors"].as_array() {
                for (at, selector) in selectors.iter().enumerate() {
                    push(
                        format!("/sections/{name}/selectors/{at}/rows"),
                        &selector["rows"],
                    );
                }
            }
            if let Some(views) = section["views"].as_object() {
                for (view, tree) in views {
                    push(
                        format!("/sections/{name}/views/{view}/nodes"),
                        &tree["nodes"],
                    );
                }
            }
        }
    }
    if let Some(views) = answer["views"].as_array() {
        for (at, view) in views.iter().enumerate() {
            if let Some(outline) = view["outline"].as_object() {
                for (name, list) in outline {
                    push(format!("/views/{at}/outline/{name}"), list);
                }
            }
        }
    }
    found
}

fn size(value: &Value) -> usize {
    serde_json::to_string(value).map_or(usize::MAX, |text| text.len())
}

/// Cuts `answer`'s longest lists until it fits `budget` bytes, or nothing is left to cut.
pub fn fit(answer: &mut Value, budget: usize) {
    while size(answer) > budget {
        let Some((pointer, length)) = lists(answer)
            .into_iter()
            .filter(|(_, length)| *length > 0)
            .max_by_key(|(_, length)| *length)
        else {
            return;
        };
        if let Some(list) = answer.pointer_mut(&pointer).and_then(Value::as_array_mut) {
            list.truncate(length / 2);
        }
        let parent = &pointer[..pointer.rfind('/').unwrap_or(0)];
        if let Some(object) = answer.pointer_mut(parent).and_then(Value::as_object_mut) {
            object.insert("cut".to_owned(), Value::Bool(true));
        } else if parent.is_empty()
            && let Some(object) = answer.as_object_mut()
        {
            object.insert("cut".to_owned(), Value::Bool(true));
        }
    }
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
