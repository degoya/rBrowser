//! The state a bug report records of each view (RB-093-04): label, breakpoint, size, engine,
//! page and title - masked - and its emulation as name/value lines ([`emulation_lines`]): what
//! is set, the extra headers by name only, an emulated position as `set`. The emulation comes in
//! as the JSON of `rb_engine::Emulation`, so this module needs no engine.

use serde_json::Value;

use super::mask::Masker;

/// The emulation fields a view's own line already shows.
const SIZE_FIELDS: [&str; 3] = ["width", "height", "dpr"];

/// One view of a report.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewFacts {
    /// The view's id.
    pub id: u32,
    /// The cell's label.
    pub label: String,
    /// The breakpoint it shows; none after a free size.
    pub breakpoint: Option<String>,
    /// Viewport width in CSS px.
    pub width: u32,
    /// Viewport height in CSS px.
    pub height: u32,
    /// Device pixel ratio.
    pub dpr: f64,
    /// `chromium`, `firefox` or `webkit`.
    pub engine: String,
    /// The page's address, masked.
    pub url: String,
    /// The page's title, masked.
    pub title: String,
    /// The emulation, [`emulation_lines`].
    pub emulation: Vec<(String, String)>,
}

/// `emulation` (the JSON of `rb_engine::Emulation`) as name/value lines in its field order:
/// strings as they are, other values as compact JSON, every value masked. The size fields are
/// the view's own; the extra headers name their names only, sorted; an emulated position says
/// `set`.
pub fn emulation_lines(emulation: &Value, masker: &Masker) -> Vec<(String, String)> {
    let Some(fields) = emulation.as_object() else {
        return Vec::new();
    };
    fields
        .iter()
        .filter(|(name, value)| !SIZE_FIELDS.contains(&name.as_str()) && !value.is_null())
        .map(|(name, value)| {
            let shown = match (name.as_str(), value) {
                ("headers", Value::Object(headers)) => {
                    let mut names: Vec<&str> = headers.keys().map(String::as_str).collect();
                    names.sort_unstable();
                    names.join(", ")
                }
                ("geolocation", _) => "set".to_owned(),
                (_, Value::String(text)) => text.clone(),
                (_, other) => other.to_string(),
            };
            (name.clone(), masker.text(&shown))
        })
        .collect()
}

/// The values of `emulation`'s extra headers: secrets of the report wherever they appear.
pub fn header_values(emulation: &Value) -> Vec<String> {
    emulation["headers"]
        .as_object()
        .map(|headers| {
            headers
                .values()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
