//! One view's outline as the collector (`web/src/collect.ts`, built into `src/collect.js`)
//! reads it through `evaluate`, held to the plugin's limits again: the page shapes all of it, so
//! every text is cut, every list capped, every number made finite.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::settings::CustomSelector;

/// The collector, defining the global `OutlineCollect`.
const COLLECT_JS: &str = include_str!("collect.js");

/// The most entries of one list (the collector cuts at the same number).
pub const MAX_ENTRIES: usize = 500;
/// The most hits listed per custom selector.
pub const MAX_HITS: usize = 100;
/// The most pieces of content outside every landmark listed (the collector cuts at the same).
pub const MAX_OUTSIDE: usize = 50;
/// The longest text kept, in characters.
pub const MAX_TEXT: usize = 200;
/// The longest URL, selector or path kept, in characters.
pub const MAX_URL: usize = 2048;
/// The largest number taken (a coordinate in CSS px, a count); beyond it the page lies.
const MAX_PX: f64 = 10_000_000.0;

/// The sections the collector reads (`axtree` comes from CDP, not from the page).
pub const SECTIONS: [&str; 7] = [
    "headings",
    "landmarks",
    "links",
    "images",
    "forms",
    "focus",
    "custom",
];

fn text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(cut(value.as_str().unwrap_or_default(), MAX_TEXT))
}

fn long<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = Value::deserialize(deserializer)?;
    Ok(cut(value.as_str().unwrap_or_default(), MAX_URL))
}

/// A whole number of the page: finite, rounded and clamped; anything else is zero.
fn whole(value: &Value) -> i64 {
    value
        .as_f64()
        .filter(|n| n.is_finite())
        .map_or(0, |n| n.clamp(-MAX_PX, MAX_PX).round() as i64)
}

fn number<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    Ok(whole(&Value::deserialize(deserializer)?))
}

fn boxed<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<[i64; 4], D::Error> {
    let value = Value::deserialize(deserializer)?;
    let at = |index: usize| value.get(index).map_or(0, whole);
    Ok([at(0), at(1), at(2), at(3)])
}

/// `text` cut to `max` characters.
#[must_use]
pub fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// What every entry carries: where it is and whether it shows.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Located {
    #[serde(deserialize_with = "long")]
    pub selector: String,
    #[serde(deserialize_with = "long")]
    pub path: String,
    /// Not exposed at all (`display: none`, `hidden`, ...).
    pub hidden: bool,
    /// On the screen.
    pub visible: bool,
    #[serde(rename = "box", deserialize_with = "boxed")]
    pub rect: [i64; 4],
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Heading {
    #[serde(flatten)]
    pub at: Located,
    /// Its place in the document, comparable with a focusable element's (RB-094-02).
    #[serde(deserialize_with = "number")]
    pub order: i64,
    #[serde(deserialize_with = "number")]
    pub level: i64,
    #[serde(deserialize_with = "text")]
    pub text: String,
    /// The accessible name, the alts of its images included; '' when it has none.
    #[serde(deserialize_with = "text")]
    pub name: String,
    /// The text's full length in characters (`text` is cut).
    #[serde(deserialize_with = "number")]
    pub length: i64,
    /// It holds an image without an alt attribute.
    pub image: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Landmark {
    #[serde(flatten)]
    pub at: Located,
    #[serde(deserialize_with = "text")]
    pub role: String,
    #[serde(deserialize_with = "text")]
    pub tag: String,
    #[serde(deserialize_with = "text")]
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Link {
    #[serde(flatten)]
    pub at: Located,
    #[serde(deserialize_with = "long")]
    pub href: String,
    #[serde(deserialize_with = "text")]
    pub text: String,
    pub internal: bool,
    pub blank_unsafe: bool,
    pub nofollow: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Image {
    #[serde(flatten)]
    pub at: Located,
    #[serde(deserialize_with = "long")]
    pub src: String,
    /// `None` when the attribute is missing.
    pub alt: Option<String>,
    pub decorative: bool,
    #[serde(deserialize_with = "number")]
    pub width: i64,
    #[serde(deserialize_with = "number")]
    pub height: i64,
    #[serde(deserialize_with = "number")]
    pub natural_width: i64,
    #[serde(deserialize_with = "number")]
    pub natural_height: i64,
    pub lazy: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Field {
    #[serde(flatten)]
    pub at: Located,
    #[serde(deserialize_with = "text")]
    pub tag: String,
    #[serde(rename = "type", deserialize_with = "text")]
    pub kind: String,
    #[serde(deserialize_with = "text")]
    pub name: String,
    #[serde(deserialize_with = "text")]
    pub label: String,
    #[serde(deserialize_with = "text")]
    pub label_source: String,
    #[serde(deserialize_with = "text")]
    pub autocomplete: String,
    pub required: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Focusable {
    #[serde(flatten)]
    pub at: Located,
    /// Its place in the document (the focus list is in focus order).
    #[serde(deserialize_with = "number")]
    pub order: i64,
    #[serde(deserialize_with = "text")]
    pub tag: String,
    #[serde(deserialize_with = "text")]
    pub role: String,
    #[serde(deserialize_with = "text")]
    pub name: String,
    #[serde(deserialize_with = "number")]
    pub tabindex: i64,
    /// Fixed or sticky on the screen (itself or an ancestor): no place in the reading order.
    pub fixed: bool,
}

/// Visible content outside every landmark: the outermost element of it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Outside {
    #[serde(flatten)]
    pub at: Located,
    #[serde(deserialize_with = "text")]
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomHit {
    #[serde(flatten)]
    pub at: Located,
    #[serde(deserialize_with = "text")]
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomResult {
    #[serde(deserialize_with = "text")]
    pub label: String,
    #[serde(deserialize_with = "long")]
    pub selector: String,
    pub invalid: bool,
    #[serde(deserialize_with = "number")]
    pub count: i64,
    pub hits: Vec<CustomHit>,
}

/// One view's outline.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PageOutline {
    #[serde(deserialize_with = "long")]
    pub url: String,
    #[serde(deserialize_with = "text")]
    pub lang: String,
    /// The writing direction of the content, `ltr` or `rtl` ([`PageOutline::is_rtl`]).
    #[serde(deserialize_with = "text")]
    pub dir: String,
    pub headings: Vec<Heading>,
    pub landmarks: Vec<Landmark>,
    /// Read with the landmarks.
    pub outside: Vec<Outside>,
    pub links: Vec<Link>,
    pub images: Vec<Image>,
    pub fields: Vec<Field>,
    pub focus: Vec<Focusable>,
    pub custom: Vec<CustomResult>,
    pub truncated: Vec<String>,
}

impl PageOutline {
    /// The content reads right to left: the reading order runs from the right.
    #[must_use]
    pub fn is_rtl(&self) -> bool {
        self.dir == "rtl"
    }

    /// The collector's answer, its lists capped.
    ///
    /// # Errors
    ///
    /// `plugins.error.bad_output` when the answer is no outline.
    pub fn parse(answer: &str) -> Result<Self, Failure> {
        let bad = || Failure::coded(FailureKind::BadData, rb_plugin_api::codes::BAD_OUTPUT);
        let value: Value = serde_json::from_str(answer).map_err(|_| bad())?;
        if !value.is_object() {
            return Err(bad());
        }
        let mut outline: Self = serde_json::from_value(value).map_err(|_| bad())?;
        outline.headings.truncate(MAX_ENTRIES);
        outline.landmarks.truncate(MAX_ENTRIES);
        outline.outside.truncate(MAX_OUTSIDE);
        outline.links.truncate(MAX_ENTRIES);
        outline.images.truncate(MAX_ENTRIES);
        outline.fields.truncate(MAX_ENTRIES);
        outline.focus.truncate(MAX_ENTRIES);
        outline.custom.truncate(crate::settings::MAX_CUSTOM);
        for result in &mut outline.custom {
            result.hits.truncate(MAX_HITS);
        }
        for image in &mut outline.images {
            if let Some(alt) = image.alt.as_mut() {
                *alt = cut(alt, MAX_TEXT);
            }
        }
        outline.truncated.truncate(SECTIONS.len());
        for name in &mut outline.truncated {
            *name = cut(name, 16);
        }
        Ok(outline)
    }
}

/// The expression that collects `sections` (every one when empty) with `custom` in a view.
#[must_use]
pub fn script(sections: &[String], custom: &[CustomSelector]) -> String {
    let request = json!({ "sections": sections, "custom": custom });
    // The request is JSON text: data, never code, inside the expression.
    format!("(() => {{ {COLLECT_JS}\n; return OutlineCollect.run({request}); }})()")
}

/// `view`'s outline of `sections` with the custom selectors `custom`.
///
/// # Errors
///
/// The host's failure to evaluate (no such view, not Chromium) or a malformed answer.
pub fn collect(
    host: &mut dyn PluginHost,
    view: ViewId,
    sections: &[String],
    custom: &[CustomSelector],
) -> Result<PageOutline, Failure> {
    let answer = host.evaluate(view, &script(sections, custom))?;
    PageOutline::parse(&answer)
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
