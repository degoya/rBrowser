//! The overflow measurement: the script `overlay_overflow` evaluates in a view, its report and
//! the log lines a report makes.
//!
//! The script is `web/src/measure.ts`, bundled into `assets/measure.js` (`npm run build` in
//! `web/`); the page script draws with the same measurement. A report comes from the page, so
//! every list and text is bounded here before it reaches an answer or the log.

use serde::{Deserialize, Serialize};

/// The bundled measurement: defines `__rbOverlaysMeasure` with `measure(options)`.
pub const MEASURE_JS: &str = include_str!("../assets/measure.js");
/// The most causes and `100vw` elements a report keeps.
pub const MAX_MARKS: usize = 50;
/// The longest selector a report keeps, in characters.
pub const MAX_SELECTOR_CHARS: usize = 300;
/// The most causes one log line names.
const LOGGED_CAUSES: usize = 5;

/// The expression `evaluate` runs: the measurement's answer, an object.
#[must_use]
pub fn measure_script(show100vw: bool) -> String {
    format!(
        "(() => {{\n{MEASURE_JS}\nreturn __rbOverlaysMeasure.measure({{ show100vw: {show100vw} }});\n}})()"
    )
}

/// A box in CSS px, relative to the document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// An element the report names.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mark {
    pub selector: String,
    pub rect: Rect,
    /// How far its right edge passes the viewport, in whole px (causes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overhang: Option<i64>,
}

/// One view's overflow.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Report {
    pub has_horizontal_scroll: bool,
    pub scroll_width: f64,
    pub client_width: f64,
    pub inner_width: f64,
    /// The outermost elements whose right edge passes the viewport, not clipped by an ancestor.
    pub causes: Vec<Mark>,
    /// Elements as wide as the viewport including its scrollbar (`100vw`).
    pub vw100: Vec<Mark>,
}

impl Report {
    /// The report in `json` (an object, or the same as JSON text); bounded.
    #[must_use]
    pub fn parse(json: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        let value = match value {
            serde_json::Value::String(text) => serde_json::from_str(&text).ok()?,
            other => other,
        };
        let report: Self = serde_json::from_value(value).ok()?;
        Some(report.bounded())
    }

    /// The report with its lists and selectors cut to their bounds, numbers made finite.
    #[must_use]
    pub fn bounded(mut self) -> Self {
        for list in [&mut self.causes, &mut self.vw100] {
            list.truncate(MAX_MARKS);
            for mark in list.iter_mut() {
                mark.selector = clean(&mark.selector);
                mark.rect = Rect {
                    x: finite(mark.rect.x),
                    y: finite(mark.rect.y),
                    width: finite(mark.rect.width),
                    height: finite(mark.rect.height),
                };
            }
        }
        self.scroll_width = finite(self.scroll_width);
        self.client_width = finite(self.client_width);
        self.inner_width = finite(self.inner_width);
        self
    }

    /// The log line for view `view`: the scroll and its causes; none without a scroll.
    #[must_use]
    pub fn log_line(&self, view: u32) -> Option<String> {
        if !self.has_horizontal_scroll {
            return None;
        }
        let causes: Vec<String> = self
            .causes
            .iter()
            .take(LOGGED_CAUSES)
            .map(|mark| format!("{} (+{} px)", mark.selector, mark.overhang.unwrap_or(0)))
            .collect();
        let more = self.causes.len().saturating_sub(LOGGED_CAUSES);
        let mut line = format!(
            "view {view}: horizontal scroll, scrollWidth {} > clientWidth {}",
            self.scroll_width.round(),
            self.client_width.round()
        );
        if causes.is_empty() {
            line.push_str("; no element found");
        } else {
            line.push_str("; caused by ");
            line.push_str(&causes.join(", "));
        }
        if more > 0 {
            line.push_str(&format!(" and {more} more"));
        }
        Some(line)
    }
}

/// `text` without control characters, within [`MAX_SELECTOR_CHARS`].
fn clean(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_SELECTOR_CHARS)
        .collect()
}

fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
