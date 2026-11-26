//! What `settings_get` answers (RB-030-03): the settings window's structure - categories, their
//! sections, the keys in the window's order with type, value, origin, default, label and
//! description - filtered by key, category, section or a search like the window's (every word
//! in the label, the key or the description, ignoring case). Every key carries its deep link
//! `rbrowser://settings/<key>`, so an agent can lead a person there.
//!
//! The backend hands over the whole document ([`crate::Backend::settings`]), secrets masked:
//! `{ categories: [{ id, title, sections: [{ id, title }] }], keys: [{ key, category, section,
//! ... }] }`.

use serde_json::{Map, Value, json};

use crate::backend::ToolFailure;

/// The prefix of a setting's deep link.
pub const DEEP_LINK_PREFIX: &str = "rbrowser://settings/";
/// The code of a key that is not there.
pub const KEY_NOT_FOUND: &str = "settings.key_not_found";
/// The code of an unknown category.
pub const CATEGORY_NOT_FOUND: &str = "settings.category_not_found";
/// The code of an unknown section.
pub const SECTION_NOT_FOUND: &str = "settings.section_not_found";

/// What `settings_get` was asked for.
#[derive(Clone, Copy, Debug, Default)]
pub struct Filter<'a> {
    /// One key.
    pub key: Option<&'a str>,
    /// A category id (`automation`).
    pub category: Option<&'a str>,
    /// A section id (`mcp`), inside `category` when both are given.
    pub section: Option<&'a str>,
    /// Words that must all be found.
    pub query: Option<&'a str>,
}

impl Filter<'_> {
    fn narrows(&self) -> bool {
        self.category.is_some()
            || self.section.is_some()
            || self.query.is_some_and(|query| !query.trim().is_empty())
    }
}

/// The deep link of `key`.
#[must_use]
pub fn deep_link(key: &str) -> String {
    format!("{DEEP_LINK_PREFIX}{key}")
}

/// The entry of `key` in `document`, with its deep link.
#[must_use]
pub fn entry(document: &Value, key: &str) -> Option<Value> {
    keys(document)
        .find(|entry| entry["key"] == key)
        .map(with_link)
}

/// The answer to `filter` over `document`.
///
/// # Errors
///
/// An unknown key, category or section, with what is known.
pub fn view(document: &Value, filter: &Filter<'_>) -> Result<Value, ToolFailure> {
    if let Some(key) = filter.key {
        return entry(document, key).ok_or_else(|| {
            ToolFailure::new(KEY_NOT_FOUND, format!("no setting has the key `{key}`"))
        });
    }
    let categories: Vec<&Value> = document["categories"]
        .as_array()
        .map(|all| all.iter().collect())
        .unwrap_or_default();
    if let Some(category) = filter.category
        && !categories.iter().any(|known| known["id"] == category)
    {
        let known: Vec<&str> = categories.iter().filter_map(|c| c["id"].as_str()).collect();
        return Err(ToolFailure::new(
            CATEGORY_NOT_FOUND,
            format!(
                "no settings category `{category}`; known: {}",
                known.join(", ")
            ),
        ));
    }
    let words: Vec<String> = filter
        .query
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    let mut shown = Vec::new();
    let mut section_seen = filter.section.is_none();
    let mut count = 0;
    for category in categories {
        let id = category["id"].as_str().unwrap_or_default();
        if filter.category.is_some_and(|wanted| wanted != id) {
            continue;
        }
        let mut sections = Vec::new();
        for section in category["sections"].as_array().into_iter().flatten() {
            let section_id = section["id"].as_str().unwrap_or_default();
            if filter.section.is_some_and(|wanted| wanted != section_id) {
                continue;
            }
            section_seen = true;
            let entries: Vec<Value> = keys(document)
                .filter(|entry| entry["category"] == id && entry["section"] == section_id)
                .filter(|entry| words.iter().all(|word| found(entry, word)))
                .map(with_link)
                .collect();
            count += entries.len();
            if entries.is_empty() && filter.narrows() {
                continue;
            }
            let mut section = section.as_object().cloned().unwrap_or_default();
            section.insert("keys".to_owned(), Value::Array(entries));
            sections.push(Value::Object(section));
        }
        if sections.is_empty() && filter.narrows() {
            continue;
        }
        let mut category = category.as_object().cloned().unwrap_or_else(Map::new);
        category.insert("sections".to_owned(), Value::Array(sections));
        shown.push(Value::Object(category));
    }
    if !section_seen {
        let section = filter.section.unwrap_or_default();
        return Err(ToolFailure::new(
            SECTION_NOT_FOUND,
            format!("no settings section `{section}` there"),
        ));
    }
    Ok(json!({ "categories": shown, "count": count }))
}

fn keys(document: &Value) -> impl Iterator<Item = &Value> {
    document["keys"].as_array().into_iter().flatten()
}

fn with_link(entry: &Value) -> Value {
    let mut entry = entry.clone();
    if let (Some(key), Some(object)) = (entry["key"].as_str().map(deep_link), entry.as_object_mut())
    {
        object.insert("deepLink".to_owned(), Value::String(key));
    }
    entry
}

/// Whether `word` (lower case) is in the label, the key or the description of `entry`.
fn found(entry: &Value, word: &str) -> bool {
    ["label", "key", "description"].iter().any(|field| {
        entry[*field]
            .as_str()
            .is_some_and(|text| text.to_lowercase().contains(word))
    })
}

#[cfg(test)]
#[path = "settings_view_tests.rs"]
mod tests;
