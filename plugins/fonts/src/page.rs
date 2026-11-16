//! One view's fonts as the page part (`web/src/collect.ts`, built into `src/page.js`) reads them
//! through `evaluate`, held to the plugin's limits again: the page shapes all of it, so every text
//! is cut, every list capped, every number made finite. The marks run through the same bundle.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::settings::Settings;

/// The page part, defining the global `FontsPage`.
const PAGE_JS: &str = include_str!("page.js");

/// The most faces, usages, files kept (the page part cuts at the same numbers).
pub const MAX_FACES: usize = 200;
pub const MAX_USAGES: usize = 200;
pub const MAX_RESOURCES: usize = 200;
/// The most elements per readability list.
pub const MAX_CANDIDATES: usize = 50;
/// The most samples per usage, roles per page, sources per face.
const MAX_SAMPLES: usize = 3;
const MAX_ROLES: usize = 20;
const MAX_SRC: usize = 16;
/// The longest text kept, in characters.
pub const MAX_TEXT: usize = 200;
/// The longest URL, selector or path kept, in characters.
pub const MAX_URL: usize = 2048;
/// The largest number taken (a size in px or bytes); beyond it the page lies.
const MAX_NUMBER: f64 = 1.0e12;

/// `text` cut to `max` characters.
#[must_use]
pub fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(cut(value.as_str().unwrap_or_default(), MAX_TEXT))
}

fn long<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(cut(value.as_str().unwrap_or_default(), MAX_URL))
}

/// A number of the page: finite and clamped; anything else is zero.
fn finite(value: &Value) -> f64 {
    value
        .as_f64()
        .filter(|n| n.is_finite())
        .map_or(0.0, |n| n.clamp(-MAX_NUMBER, MAX_NUMBER))
}

fn number<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(finite(&Value::deserialize(deserializer)?))
}

fn count<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    // `as` saturates; the value is finite and clamped to be non-negative.
    Ok(finite(&Value::deserialize(deserializer)?).max(0.0) as u64)
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Located {
    #[serde(deserialize_with = "long")]
    pub selector: String,
    #[serde(deserialize_with = "long")]
    pub path: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Src {
    #[serde(deserialize_with = "long")]
    pub url: String,
    #[serde(deserialize_with = "text")]
    pub format: String,
    pub local: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Face {
    #[serde(deserialize_with = "text")]
    pub family: String,
    #[serde(deserialize_with = "text")]
    pub weight: String,
    #[serde(deserialize_with = "text")]
    pub style: String,
    #[serde(deserialize_with = "text")]
    pub display: String,
    #[serde(deserialize_with = "text")]
    pub unicode_range: String,
    #[serde(deserialize_with = "text")]
    pub status: String,
    pub src: Vec<Src>,
    #[serde(deserialize_with = "long")]
    pub sheet: String,
    pub readable: bool,
    pub variable: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Resource {
    #[serde(deserialize_with = "long")]
    pub url: String,
    #[serde(deserialize_with = "count")]
    pub size: u64,
    #[serde(deserialize_with = "count")]
    pub decoded: u64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Sample {
    #[serde(flatten)]
    pub at: Located,
    #[serde(deserialize_with = "text")]
    pub text: String,
    #[serde(deserialize_with = "text")]
    pub non_ascii: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Usage {
    #[serde(deserialize_with = "long")]
    pub stack: String,
    #[serde(deserialize_with = "text")]
    pub family: String,
    #[serde(deserialize_with = "count")]
    pub weight: u64,
    #[serde(deserialize_with = "text")]
    pub style: String,
    #[serde(deserialize_with = "count")]
    pub count: u64,
    pub above_fold: bool,
    pub sizes: Vec<Value>,
    pub samples: Vec<Sample>,
}

impl Usage {
    /// The distinct font sizes, finite.
    #[must_use]
    pub fn sizes(&self) -> Vec<f64> {
        self.sizes.iter().map(finite).collect()
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Role {
    #[serde(deserialize_with = "text")]
    pub role: String,
    #[serde(deserialize_with = "count")]
    pub count: u64,
    #[serde(deserialize_with = "number")]
    pub min: f64,
    #[serde(deserialize_with = "number")]
    pub max: f64,
    pub first: Located,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Candidate {
    #[serde(flatten)]
    pub at: Located,
    #[serde(deserialize_with = "text")]
    pub tag: String,
    #[serde(deserialize_with = "number")]
    pub size: f64,
    #[serde(deserialize_with = "number")]
    pub line_height: f64,
    #[serde(deserialize_with = "text")]
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Viewport {
    #[serde(deserialize_with = "number")]
    pub width: f64,
    #[serde(deserialize_with = "number")]
    pub height: f64,
}

/// One view's fonts.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PageFonts {
    #[serde(deserialize_with = "long")]
    pub url: String,
    pub viewport: Viewport,
    pub faces: Vec<Face>,
    pub sheets: Vec<Value>,
    pub preloads: Vec<Value>,
    pub resources: Vec<Resource>,
    pub usages: Vec<Usage>,
    pub roles: Vec<Role>,
    pub small_text: Vec<Candidate>,
    pub small_inputs: Vec<Candidate>,
    pub tight_lines: Vec<Candidate>,
    #[serde(deserialize_with = "count")]
    pub elements: u64,
    pub truncated: bool,
}

/// The strings of a list of page values, cut, at most `max`.
fn strings(list: &[Value], max: usize) -> Vec<Value> {
    list.iter()
        .filter_map(Value::as_str)
        .take(max)
        .map(|text| Value::String(cut(text, MAX_URL)))
        .collect()
}

impl PageFonts {
    /// The page part's answer, its lists capped.
    ///
    /// # Errors
    ///
    /// `plugins.error.bad_output` when the answer is no fonts object.
    pub fn parse(answer: &str) -> Result<Self, Failure> {
        let bad = || Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT);
        let value: Value = serde_json::from_str(answer).map_err(|_| bad())?;
        if !value.is_object() {
            return Err(bad());
        }
        let mut fonts: Self = serde_json::from_value(value).map_err(|_| bad())?;
        fonts.faces.truncate(MAX_FACES);
        for face in &mut fonts.faces {
            face.src.truncate(MAX_SRC);
        }
        fonts.sheets = strings(&fonts.sheets, 50);
        fonts.preloads = strings(&fonts.preloads, 50);
        fonts.resources.truncate(MAX_RESOURCES);
        fonts.usages.truncate(MAX_USAGES);
        for usage in &mut fonts.usages {
            usage.samples.truncate(MAX_SAMPLES);
            usage.sizes.truncate(16);
        }
        fonts.roles.truncate(MAX_ROLES);
        fonts.small_text.truncate(MAX_CANDIDATES);
        fonts.small_inputs.truncate(MAX_CANDIDATES);
        fonts.tight_lines.truncate(MAX_CANDIDATES);
        Ok(fonts)
    }

    /// The preloaded URLs.
    #[must_use]
    pub fn preloaded(&self, url: &str) -> bool {
        self.preloads
            .iter()
            .any(|preload| preload.as_str() == Some(url))
    }
}

/// The expression calling `FontsPage.<function>` with `request`.
fn call(function: &str, request: &Value) -> String {
    // The request is JSON text: data, never code, inside the expression.
    format!("(() => {{ {PAGE_JS}\n; return FontsPage.{function}({request}); }})()")
}

/// The expression that collects a view's fonts with the thresholds of `settings`.
#[must_use]
pub fn script(settings: &Settings) -> String {
    call(
        "run",
        &json!({
            "minTextPx": settings.min_text_px,
            "minInputPx": settings.min_input_px,
            "minLineHeight": settings.min_line_height,
        }),
    )
}

/// `view`'s fonts.
///
/// # Errors
///
/// The host's failure to evaluate (no such view, not Chromium) or a malformed answer.
pub fn collect(
    host: &mut dyn PluginHost,
    view: ViewId,
    settings: &Settings,
) -> Result<PageFonts, Failure> {
    let answer = host.evaluate(view, &script(settings))?;
    PageFonts::parse(&answer)
}

/// Marks the text set in `family` (and `weight`, `style`) in `view`; how many elements.
///
/// # Errors
///
/// The host's failure to evaluate.
pub fn mark(
    host: &mut dyn PluginHost,
    view: ViewId,
    family: &str,
    weight: Option<u64>,
    style: Option<&str>,
) -> Result<u64, Failure> {
    let request = json!({ "family": family, "weight": weight, "style": style });
    let answer = host.evaluate(view, &call("mark", &request))?;
    let value: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
    Ok(value["count"].as_u64().unwrap_or(0))
}

/// Removes the marks in `view`.
///
/// # Errors
///
/// The host's failure to evaluate.
pub fn clear(host: &mut dyn PluginHost, view: ViewId) -> Result<(), Failure> {
    host.evaluate(view, &call("clear", &json!(null)))
        .map(|_| ())
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
