//! The fill's report to the recorder (RB-090-78). The autofill sets values by script, and the
//! sync agent records trusted input only - so a recording would lack the fill. The page script's
//! `apply` therefore names, per field it set, what the recorder takes (`record`: the field's
//! target in the shape the sync agent gives an element, what was done, the value set - a
//! password's never). Each view's list leaves the answer and goes to the core as `host` posts
//! `{"action": "record_fill", "view": <id>, "fields": [...]}`; the core turns them into steps
//! while a recording runs with `automation.recorder.recordAutofill`, and does nothing otherwise.
//! A post is held under the host's limit for one message; a view with many fields gets several.

use serde_json::{Value, json};

/// The host action the core records a fill with.
pub const RECORD_FILL: &str = "record_fill";
/// The largest message of one post, as JSON text: the host takes up to 64 KiB.
pub const MAX_MESSAGE: usize = 60_000;
/// The most record posts of one answer: the host takes 16 posts, one is the panel's.
pub const MAX_POSTS: usize = 15;

fn post(view: &Value, fields: Vec<Value>) -> Value {
    json!({ "target": "host",
            "message": { "action": RECORD_FILL, "view": view, "fields": fields } })
}

/// Takes each view's `record` list out of `answer` and makes the host posts of it, in the order
/// of the views and fields. A field that alone is over [`MAX_MESSAGE`] is left out, and so is
/// everything past [`MAX_POSTS`].
pub fn take_posts(answer: &mut Value) -> Vec<Value> {
    let mut posts = Vec::new();
    let Some(views) = answer.get_mut("views").and_then(Value::as_array_mut) else {
        return posts;
    };
    for view in views {
        let Some(Value::Array(fields)) = view.as_object_mut().and_then(|v| v.remove("record"))
        else {
            continue;
        };
        let id = view["view"].clone();
        // The message around the fields, and a comma between two.
        let frame = post(&id, Vec::new())["message"].to_string().len();
        let (mut chunk, mut size) = (Vec::new(), frame);
        for field in fields.into_iter().filter(Value::is_object) {
            let length = field.to_string().len() + 1;
            if frame + length > MAX_MESSAGE {
                continue;
            }
            if size + length > MAX_MESSAGE {
                posts.push(post(&id, std::mem::take(&mut chunk)));
                size = frame;
            }
            size += length;
            chunk.push(field);
        }
        if !chunk.is_empty() {
            posts.push(post(&id, chunk));
        }
    }
    posts.truncate(MAX_POSTS);
    posts
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
