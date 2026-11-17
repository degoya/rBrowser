//! The web app manifest as the Web App Manifest spec reads it: `start_url` and `scope` resolved
//! against the manifest's URL (a `start_url` of another origin than the document's is ignored),
//! `scope` defaulting to the start URL's directory, a member of the wrong type or a blank string
//! counting as missing, the icons with their `sizes`, `type` and `purpose`. And a short table of
//! every member for the panel.

use serde_json::{Map, Value};

use crate::image::{Sizes, parse_sizes};
use crate::url::Url;

/// The longest value shown in a table cell, in characters.
pub const MAX_CELL_CHARS: usize = 200;
/// The most members the table shows.
pub const MAX_TABLE_ROWS: usize = 64;
/// The most icons read.
pub const MAX_ICONS: usize = 32;
/// The display modes that make an app window (Chromium's installability criteria).
pub const APP_DISPLAYS: &[&str] = &["fullscreen", "standalone", "minimal-ui"];

/// One entry of `icons`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    /// The position in `icons`.
    pub index: usize,
    /// `src` as written.
    pub src: String,
    /// `src` resolved; `None` for anything but `http(s)` (a `data:` icon is not fetched).
    pub url: Option<Url>,
    /// `sizes` as written.
    pub sizes_text: String,
    pub sizes: Sizes,
    /// `type`.
    pub kind: Option<String>,
    /// `purpose`, split; `any` when none is given.
    pub purpose: Vec<String>,
}

impl Icon {
    #[must_use]
    pub fn maskable(&self) -> bool {
        self.purpose.iter().any(|purpose| purpose == "maskable")
    }
}

/// The parsed manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub name: Option<String>,
    pub short_name: Option<String>,
    /// The start URL in effect (the document's URL when none is declared or it is ignored).
    pub start_url: Url,
    pub start_url_declared: bool,
    /// `start_url` was declared but invalid or of another origin than the document.
    pub start_url_ignored: bool,
    pub scope: Url,
    pub scope_declared: bool,
    pub display: Option<String>,
    pub display_override: Vec<String>,
    pub icons: Vec<Icon>,
    pub theme_color: Option<String>,
    pub background_color: Option<String>,
    pub id: Option<String>,
    /// `(member, value)` for the panel's table, sorted by member.
    pub table: Vec<(String, String)>,
}

/// Why a manifest's text is no manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Not JSON: the parser's message.
    Json(String),
    /// JSON, but no object.
    NotObject,
}

fn text(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// `text` cut to `max` characters, `...` marking the cut.
#[must_use]
pub fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut short: String = text.chars().take(max).collect();
        short.push_str("...");
        short
    }
}

fn cell(value: &Value) -> String {
    match value {
        Value::String(text) => cut(text, MAX_CELL_CHARS),
        Value::Array(items) => format!("[{}]", items.len()),
        Value::Object(_) => cut(&value.to_string(), MAX_CELL_CHARS),
        other => other.to_string(),
    }
}

fn icons(object: &Map<String, Value>, base: &Url) -> Vec<Icon> {
    let Some(entries) = object.get("icons").and_then(Value::as_array) else {
        return Vec::new();
    };
    entries
        .iter()
        .take(MAX_ICONS)
        .enumerate()
        .filter_map(|(index, entry)| {
            let entry = entry.as_object()?;
            let src = text(entry, "src")?;
            let sizes_text = text(entry, "sizes").unwrap_or_default();
            let purpose: Vec<String> = text(entry, "purpose")
                .map(|purpose| {
                    purpose
                        .split_ascii_whitespace()
                        .map(str::to_ascii_lowercase)
                        .collect()
                })
                .unwrap_or_else(|| vec!["any".to_owned()]);
            Some(Icon {
                index,
                url: base.join(&src),
                src: cut(&src, MAX_CELL_CHARS),
                sizes: parse_sizes(&sizes_text),
                sizes_text: cut(&sizes_text, MAX_CELL_CHARS),
                kind: text(entry, "type"),
                purpose,
            })
        })
        .collect()
}

impl Manifest {
    /// `json`, the manifest served at `manifest_url` for the page at `document`.
    ///
    /// # Errors
    ///
    /// The text is no JSON, or no JSON object.
    pub fn parse(json: &str, manifest_url: &Url, document: &Url) -> Result<Self, ParseError> {
        let value: Value = serde_json::from_str(json.trim_start_matches('\u{feff}'))
            .map_err(|error| ParseError::Json(cut(&error.to_string(), MAX_CELL_CHARS)))?;
        let Value::Object(object) = value else {
            return Err(ParseError::NotObject);
        };
        let declared_start = text(&object, "start_url");
        let resolved_start = declared_start
            .as_deref()
            .and_then(|start| manifest_url.join(start))
            .filter(|start| start.same_origin(document));
        let start_url_ignored = declared_start.is_some() && resolved_start.is_none();
        let start_url = resolved_start.unwrap_or_else(|| document.clone());
        let declared_scope = text(&object, "scope").and_then(|scope| manifest_url.join(&scope));
        let scope_declared = declared_scope.is_some();
        let scope = declared_scope.unwrap_or_else(|| start_url.directory());
        let display_override = object
            .get("display_override")
            .and_then(Value::as_array)
            .map(|modes| {
                modes
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .take(16)
                    .collect()
            })
            .unwrap_or_default();
        let table = object
            .iter()
            .take(MAX_TABLE_ROWS)
            .map(|(key, value)| (cut(key, 64), cell(value)))
            .collect();
        Ok(Self {
            name: text(&object, "name"),
            short_name: text(&object, "short_name"),
            start_url_declared: declared_start.is_some(),
            start_url_ignored,
            start_url,
            scope,
            scope_declared,
            display: text(&object, "display"),
            display_override,
            icons: icons(&object, manifest_url),
            theme_color: text(&object, "theme_color"),
            background_color: text(&object, "background_color"),
            id: text(&object, "id"),
            table,
        })
    }

    /// Whether `display` or a `display_override` entry opens an app window.
    #[must_use]
    pub fn installable_display(&self) -> bool {
        self.display_override
            .iter()
            .map(String::as_str)
            .chain(self.display.as_deref())
            .any(|mode| APP_DISPLAYS.contains(&mode))
    }
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
