//! Answers that fit a panel's message (the owner's acceptance of RB-110-13, 2026-11-16): a
//! panel's call and its answer are at most 64 KiB of JSON (`MAX_MESSAGE_BYTES` of the host and of
//! the panels window), and a project's todo files are larger. So `list` answers in pages, each
//! within [`PAGE_BYTES`]: the files' heads (path, rev, counts) from file `from`, or one file's
//! items from item `from`, with `next` - the `from` to go on with - while more remain. An item
//! larger than [`MAX_ITEM_BYTES`] (a giant line, or a line below a giant project) is cut: its
//! texts and tags shortened, `cut: true`, so every page holds one item at least and moves on.

use serde_json::{Value, json};

use crate::format::{Document, revision};
use crate::items::{counts, items};

/// The most JSON bytes of one page: room below the 64 KiB message for the envelope around it
/// (the call's id, a change's own fields, the panel's post).
pub const PAGE_BYTES: usize = 48 * 1024;
/// The largest item answered whole, in JSON bytes; a larger one is cut.
pub const MAX_ITEM_BYTES: usize = 8 * 1024;
/// What a cut item keeps of each of its texts (`raw`, `text`, `project`), and of its tags
/// together, in JSON bytes.
pub const CUT_BYTES: usize = 2 * 1024;
/// Room kept for the fields a page adds after its entries (`next`, `truncated`).
const TAIL_BYTES: usize = 64;
/// The room of a list of entries (the files' heads) in one page.
pub const ENTRIES_BYTES: usize = PAGE_BYTES - TAIL_BYTES;

/// The length of `value` as JSON text.
#[must_use]
pub fn json_len(value: &Value) -> usize {
    value.to_string().len()
}

/// The bytes `c` takes inside a JSON string, as serde writes it.
fn escaped_len(c: char) -> usize {
    match c {
        '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
        c if u32::from(c) < 0x20 => 6,
        c => c.len_utf8(),
    }
}

/// The longest start of `text` that takes at most `budget` bytes inside a JSON string, ending on
/// a whole character.
#[must_use]
pub fn prefix(text: &str, budget: usize) -> &str {
    let mut used = 0;
    for (at, c) in text.char_indices() {
        used += escaped_len(c);
        if used > budget {
            return text.get(..at).unwrap_or_default();
        }
    }
    text
}

/// `item` as a page carries it: whole, or - larger than [`MAX_ITEM_BYTES`] - with its texts cut
/// to [`CUT_BYTES`] each, the tags that fit in [`CUT_BYTES`] and `cut: true`. A cut item is
/// shown, checked, moved and deleted like any other, but its text is not the line's: the panel
/// does not offer to edit it.
#[must_use]
pub fn fit(mut item: Value) -> Value {
    if json_len(&item) <= MAX_ITEM_BYTES {
        return item;
    }
    for key in ["raw", "text", "project"] {
        if let Some(text) = item[key].as_str() {
            let kept = prefix(text, CUT_BYTES).to_owned();
            item[key] = json!(kept);
        }
    }
    let mut used = 0;
    let tags: Vec<Value> = item["tags"]
        .as_array()
        .map(|tags| {
            tags.iter()
                .take_while(|tag| {
                    used += json_len(tag) + 1;
                    used <= CUT_BYTES
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    item["tags"] = json!(tags);
    item["cut"] = json!(true);
    item
}

/// The entries `make` answers for the indexes `from..count`, as many as `budget` JSON bytes hold
/// (one at least, so a page always moves on), and the index to go on from while more remain.
pub fn take(
    count: usize,
    from: usize,
    budget: usize,
    mut make: impl FnMut(usize) -> Value,
) -> (Vec<Value>, Option<usize>) {
    let mut used = 0;
    let mut taken = Vec::new();
    for index in from..count {
        let entry = make(index);
        let size = json_len(&entry) + 1;
        if !taken.is_empty() && used + size > budget {
            return (taken, Some(index));
        }
        used += size;
        taken.push(entry);
    }
    (taken, None)
}

/// The head of todo file `path` with `text`: its path, rev and counts, no items.
#[must_use]
pub fn head(path: &str, text: &str) -> Value {
    let all = items(&Document::parse(text));
    json!({ "path": path, "rev": revision(text.as_bytes()), "counts": counts(&all) })
}

/// A page of todo file `path` with `text`: its path, rev and counts, its items from item `from`
/// as many as [`PAGE_BYTES`] hold, and `next` while more remain. A `from` past the last item is
/// an empty page.
#[must_use]
pub fn file_page(path: &str, text: &str, from: usize) -> Value {
    let all = items(&Document::parse(text));
    let mut page = json!({
        "path": path,
        "rev": revision(text.as_bytes()),
        "counts": counts(&all),
        "items": [],
    });
    let budget = PAGE_BYTES.saturating_sub(json_len(&page) + TAIL_BYTES);
    let (taken, next) = take(all.len(), from, budget, |index| {
        all.get(index).cloned().map_or(Value::Null, fit)
    });
    page["items"] = Value::Array(taken);
    if let Some(next) = next {
        page["next"] = json!(next);
    }
    page
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
