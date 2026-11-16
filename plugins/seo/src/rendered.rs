//! The rendered DOM of a view: `src/collect.js` run through `evaluate`, its document parsed
//! with the same parser as the raw HTML, the rendered sizes kept by element path.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use scraper::Html;
use serde::Deserialize;

/// The script `evaluate` runs: an expression whose value is the collection.
pub const COLLECT_JS: &str = include_str!("collect.js");

/// A large CSS background image without text over it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Background {
    pub path: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize)]
struct Size {
    path: String,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct Collected {
    url: String,
    html: String,
    #[serde(default)]
    truncated: bool,
    #[serde(default)]
    images: Vec<Size>,
    #[serde(default)]
    backgrounds: Vec<Background>,
    /// The visible text of the main content; absent from older collectors.
    #[serde(default)]
    text: Option<String>,
}

/// One view's rendered page.
#[derive(Debug)]
pub struct Rendered {
    pub view: ViewId,
    /// The URL the view shows (`location.href`).
    pub url: String,
    pub html: Html,
    /// The document was longer than the collector sends and was cut.
    pub truncated: bool,
    /// Rendered width and height by element path.
    pub sizes: BTreeMap<String, (u32, u32)>,
    pub backgrounds: Vec<Background>,
    /// The visible text of `main` (or the body), as the view renders it.
    pub text: Option<String>,
}

impl Rendered {
    /// The rendered page from the collector's answer.
    ///
    /// # Errors
    ///
    /// The answer is no collection (`plugin.seo.errors.bad_render`).
    pub fn parse(view: ViewId, answer: &str) -> Result<Self, Failure> {
        let collected: Collected = serde_json::from_str(answer).map_err(|error| {
            Failure::coded(FailureKind::BadData, "plugin.seo.errors.bad_render").with_param(
                "reason",
                error.to_string().chars().take(200).collect::<String>(),
            )
        })?;
        Ok(Self {
            view,
            url: collected.url,
            html: Html::parse_document(&collected.html),
            truncated: collected.truncated,
            sizes: collected
                .images
                .into_iter()
                .map(|size| (size.path, (size.width, size.height)))
                .collect(),
            backgrounds: collected.backgrounds,
            text: collected.text,
        })
    }

    /// The rendered page of `view`.
    ///
    /// # Errors
    ///
    /// The host refused or failed `evaluate`, or the answer is no collection.
    pub fn collect(host: &mut dyn PluginHost, view: ViewId) -> Result<Self, Failure> {
        let answer = host.evaluate(view, COLLECT_JS)?;
        Self::parse(view, &answer)
    }
}

#[cfg(test)]
#[path = "rendered_tests.rs"]
mod tests;
