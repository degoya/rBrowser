//! The checking overlays (RB-070-07): stacking contexts, unneeded scrollbars, text clipped under
//! the WCAG 1.4.12 spacing, small targets and small text. The page script draws them; when one is
//! switched on, the component also runs the same finder in each view through `evaluate` (the
//! bundle `assets/measure.js`, global `__rbOverlaysMeasure.scan`) to answer the hits and report
//! them as findings. A scan comes from the page, so every list and text is bounded here.

use rb_plugin_api::{Finding, Severity, ViewInfo};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::report::{MEASURE_JS, Rect};

/// The most hits a view's scan keeps (the page sends at most as many).
pub const MAX_ITEMS: usize = 200;
/// The most hits per view an answer lists, within [`crate::budget::SCAN_BYTES`] for every view
/// together; the count is the whole number.
pub const MAX_ANSWERED: usize = 50;
/// The longest selector, path or label kept, in characters.
pub const MAX_TEXT_CHARS: usize = 300;

/// The checking overlays and the finding rule each reports (none for `stacking`: a stacking
/// context is no fault).
const SCANS: &[(&str, Option<&str>)] = &[
    ("stacking", None),
    ("scrollbars", Some("unneeded-scrollbar")),
    ("text-spacing", Some("text-spacing-clip")),
    ("target-size", Some("target-size")),
    ("small-text", Some("small-text")),
];

/// Whether `overlay` finds elements the component can scan for.
#[must_use]
pub fn is_scan(overlay: &str) -> bool {
    SCANS.iter().any(|(id, _)| *id == overlay)
}

/// The finding rule of `overlay`, if it reports findings.
#[must_use]
pub fn rule(overlay: &str) -> Option<&'static str> {
    SCANS
        .iter()
        .find(|(id, _)| *id == overlay)
        .and_then(|(_, rule)| *rule)
}

/// The expression `evaluate` runs: the scan's answer, an object.
#[must_use]
pub fn script(overlay: &str, options: &Value) -> String {
    let overlay = Value::String(overlay.to_owned());
    format!(
        "(() => {{\n{MEASURE_JS}\nreturn __rbOverlaysMeasure.scan({overlay}, {options});\n}})()"
    )
}

/// One element a scan found.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hit {
    /// A short selector that finds it.
    pub selector: String,
    /// Its full path from the root: what makes the same element one finding in every view.
    pub path: String,
    /// What the overlay labels it with (`20x20`, `11px`, `clipped +12px`).
    pub label: String,
    pub rect: Rect,
}

/// One view's scan.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scan {
    /// Every element found, also those past [`MAX_ITEMS`].
    pub count: u64,
    pub items: Vec<Hit>,
}

fn clean(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_TEXT_CHARS)
        .collect()
}

fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

impl Scan {
    /// The scan in `json` (an object, or the same as JSON text); bounded.
    #[must_use]
    pub fn parse(json: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(json).ok()?;
        let value = match value {
            Value::String(text) => serde_json::from_str(&text).ok()?,
            other => other,
        };
        if !value.is_object() {
            return None;
        }
        let mut scan: Self = serde_json::from_value(value).ok()?;
        scan.items.truncate(MAX_ITEMS);
        for hit in &mut scan.items {
            hit.selector = clean(&hit.selector);
            hit.path = clean(&hit.path);
            hit.label = clean(&hit.label);
            hit.rect = Rect {
                x: finite(hit.rect.x),
                y: finite(hit.rect.y),
                width: finite(hit.rect.width),
                height: finite(hit.rect.height),
            };
        }
        Some(scan)
    }

    /// The answer's entry for `view`: its count and its first `shown` hits (at most
    /// [`MAX_ANSWERED`]; [`crate::budget::share`] says how many fit).
    #[must_use]
    pub fn answer(&self, view: &ViewInfo, shown: usize) -> Value {
        let items: Vec<&Hit> = self.items.iter().take(shown.min(MAX_ANSWERED)).collect();
        json!({
            "view": view.id,
            "label": view.label,
            "count": self.count,
            "shown": items.len(),
            "items": items,
        })
    }
}

/// The findings of `scan` in `view`: one per hit, for an overlay with a rule.
#[must_use]
pub fn findings(overlay: &str, view: &ViewInfo, scan: &Scan) -> Vec<Finding> {
    let Some(rule) = rule(overlay) else {
        return Vec::new();
    };
    let message = format!("plugin.overlays.findings.{}", rule.replace('-', "_"));
    scan.items
        .iter()
        .map(|hit| Finding {
            rule: rule.to_owned(),
            severity: Severity::Warning,
            message: message.clone(),
            params: vec![("label".to_owned(), hit.label.clone())],
            view: Some(view.id),
            url: (!view.url.is_empty()).then(|| view.url.clone()),
            selector: (!hit.selector.is_empty()).then(|| hit.selector.clone()),
            path: (!hit.path.is_empty()).then(|| hit.path.clone()),
            data: None,
        })
        .collect()
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
