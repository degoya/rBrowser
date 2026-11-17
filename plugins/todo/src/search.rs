//! `list`'s `query` (RB-110-18), the panel's search rules (`web/src/panel/search.ts`): the terms
//! split at white space, each without case and accents ([`fold`]); a term `@name` is a tag whose
//! name starts with `name`, any other a word that the item's text, a tag's name or a tag's value
//! contains. An item matches when every term does; the answer keeps each match's headings,
//! projects and parent lines above it as context, each item saying whether it matched.

use serde_json::{Value, json};

use crate::edit::ARCHIVE;
use crate::fold::fold;

/// The longest `query`, in characters.
pub const MAX_QUERY_CHARS: usize = 200;

/// The terms of a search, folded.
#[derive(Debug, PartialEq, Eq)]
pub struct Query {
    /// The words.
    pub words: Vec<String>,
    /// The tag terms' names without `@`.
    pub tags: Vec<String>,
}

impl Query {
    /// The terms of `text`; `@` alone and terms that fold to nothing are left out. `None` when no
    /// term is left: no search.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut query = Self {
            words: Vec::new(),
            tags: Vec::new(),
        };
        for term in text.split_whitespace() {
            if let Some(tag) = term.strip_prefix('@') {
                let name = fold(tag.split('(').next().unwrap_or_default());
                if !name.is_empty() {
                    query.tags.push(name);
                }
            } else {
                let word = fold(term);
                if !word.is_empty() {
                    query.words.push(word);
                }
            }
        }
        (!query.words.is_empty() || !query.tags.is_empty()).then_some(query)
    }

    /// Whether `item` (an entry of [`crate::items::items`]) matches every term.
    #[must_use]
    pub fn matches(&self, item: &Value) -> bool {
        let text = fold(item["text"].as_str().unwrap_or_default());
        let tags: Vec<(String, String)> = item["tags"]
            .as_array()
            .map(|tags| {
                tags.iter()
                    .map(|tag| {
                        (
                            fold(tag["name"].as_str().unwrap_or_default()),
                            fold(tag["value"].as_str().unwrap_or_default()),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.words.iter().all(|word| {
            text.contains(word.as_str())
                || tags.iter().any(|(name, value)| {
                    name.contains(word.as_str()) || value.contains(word.as_str())
                })
        }) && self
            .tags
            .iter()
            .all(|term| tags.iter().any(|(name, _)| name.starts_with(term.as_str())))
    }
}

fn depth(item: &Value) -> u64 {
    item["depth"].as_u64().unwrap_or_default()
}

fn is_heading(item: &Value) -> bool {
    item["kind"] == "heading"
}

fn level(item: &Value) -> u64 {
    item["level"].as_u64().unwrap_or(1)
}

/// Whether `head` holds `item`, the next line after everything it held so far: a deeper line, or
/// - for a Markdown heading - a line at its depth that is no heading of its level or a higher one.
fn holds(head: &Value, item: &Value) -> bool {
    if depth(head) < depth(item) {
        return true;
    }
    if !is_heading(head) || depth(head) != depth(item) {
        return false;
    }
    !is_heading(item) || level(item) > level(head)
}

/// Whether `item` is the top-level `Archive:`, which no heading above it holds.
fn is_archive_head(item: &Value) -> bool {
    item["kind"] == "project" && depth(item) == 0 && item["text"] == ARCHIVE
}

/// The items of `items` that match `query`, each with the lines that hold it as context, in file
/// order and each with `match` (`true` for a match, `false` for context); and how many matched.
/// One pass: the lines that hold the current one are a stack.
#[must_use]
pub fn filter(items: &[Value], query: &Query) -> (Vec<Value>, usize) {
    let mut shown = vec![None; items.len()];
    let mut stack: Vec<usize> = Vec::new();
    let mut matched = 0;
    for (index, item) in items.iter().enumerate() {
        if is_archive_head(item) {
            stack.clear();
        }
        while stack
            .last()
            .and_then(|&head| items.get(head))
            .is_some_and(|head| !holds(head, item))
        {
            stack.pop();
        }
        if query.matches(item) {
            matched += 1;
            // Upwards until a line already shown: its own context is shown with it.
            for &head in stack.iter().rev() {
                let Some(slot) = shown.get_mut(head) else {
                    break;
                };
                if slot.is_some() {
                    break;
                }
                *slot = Some(false);
            }
            if let Some(slot) = shown.get_mut(index) {
                *slot = Some(true);
            }
        }
        stack.push(index);
    }
    let kept = items
        .iter()
        .zip(shown)
        .filter_map(|(item, shown)| {
            shown.map(|hit| {
                let mut item = item.clone();
                item["match"] = json!(hit);
                item
            })
        })
        .collect();
    (kept, matched)
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
