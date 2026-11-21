//! One view's images, as the collector (`src/collect.js`, through `evaluate` in the plugin's
//! isolated world, read only) reads and measures them at the view's density: each `<img>` with its
//! chosen file, its own pixels, its rendered and needed size and its issues.
//!
//! The answer comes from a page, so it is held to the plugin's own limits here: at most
//! [`MAX_IMAGES`] images, texts cut, only the known rules, variants and parameters, numbers
//! finite.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::rules;
use crate::settings::Settings;

/// The collector, defining the global `ResponsiveImagesCollect`.
pub const COLLECT_JS: &str = include_str!("collect.js");
/// The most images taken of one view (the collector's `MAX_IMAGES`).
pub const MAX_IMAGES: usize = 300;
/// The longest text kept: a URL, an attribute, a selector.
pub const MAX_TEXT: usize = 1000;

/// A size in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// The `<source>` of a `<picture>` the image came from.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Source {
    pub media: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub srcset: String,
    pub sizes: String,
}

/// One problem of one image (`web/src/analyze.ts`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Issue {
    pub rule: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    pub params: BTreeMap<String, String>,
}

/// One `<img>` of the page, measured at the view's density.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Image {
    pub selector: String,
    /// Lighthouse's node path, the element of a finding.
    pub path: String,
    pub src: String,
    pub current_src: String,
    pub srcset: String,
    pub sizes: String,
    pub picture: bool,
    pub source: Option<Source>,
    pub source_without_sizes: bool,
    pub loading: String,
    pub fetchpriority: String,
    pub rendered: Size,
    pub natural: Size,
    pub file: Option<Size>,
    pub file_from: Option<String>,
    pub vector: bool,
    pub loaded: bool,
    pub visible: bool,
    pub lcp: bool,
    pub needed: Option<Size>,
    pub ratio: Option<f64>,
    pub issues: Vec<Issue>,
}

/// One view's images.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PageImages {
    pub url: String,
    pub dpr: f64,
    pub viewport: Size,
    pub images: Vec<Image>,
    /// Images beyond those listed.
    pub more: u64,
    /// Whether the page named its LCP element (else the largest image of the first viewport).
    pub lcp_known: bool,
}

fn cut(text: &mut String) {
    if let Some((index, _)) = text.char_indices().nth(MAX_TEXT) {
        text.truncate(index);
    }
}

fn finite(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn size(size: &mut Size) {
    size.width = finite(size.width);
    size.height = finite(size.height);
}

impl Image {
    /// The image held to the plugin's limits; an issue of an unknown rule, variant or parameter
    /// left out.
    fn sanitized(mut self) -> Self {
        for text in [
            &mut self.selector,
            &mut self.path,
            &mut self.src,
            &mut self.current_src,
            &mut self.srcset,
            &mut self.sizes,
            &mut self.loading,
            &mut self.fetchpriority,
        ] {
            cut(text);
        }
        if let Some(source) = self.source.as_mut() {
            for text in [
                &mut source.media,
                &mut source.kind,
                &mut source.srcset,
                &mut source.sizes,
            ] {
                cut(text);
            }
        }
        size(&mut self.rendered);
        size(&mut self.natural);
        for measured in [self.file.as_mut(), self.needed.as_mut()]
            .into_iter()
            .flatten()
        {
            size(measured);
        }
        self.ratio = self
            .ratio
            .filter(|ratio| ratio.is_finite() && *ratio >= 0.0);
        if self
            .file_from
            .as_deref()
            .is_some_and(|from| !["probe", "descriptor", "natural"].contains(&from))
        {
            self.file_from = None;
        }
        self.issues = self
            .issues
            .into_iter()
            .filter_map(rules::known_issue)
            .collect();
        self
    }
}

impl PageImages {
    /// The answer held to the plugin's limits.
    #[must_use]
    pub fn sanitized(mut self) -> Self {
        cut(&mut self.url);
        self.dpr = finite(self.dpr);
        size(&mut self.viewport);
        if self.images.len() > MAX_IMAGES {
            let dropped = self.images.len() - MAX_IMAGES;
            self.more = self
                .more
                .saturating_add(u64::try_from(dropped).unwrap_or(u64::MAX));
            self.images.truncate(MAX_IMAGES);
        }
        self.images = self.images.into_iter().map(Image::sanitized).collect();
        self
    }
}

/// The collector's request: the view's density (the page's own when the view has none) and the
/// limits of the settings.
#[must_use]
pub fn request(dpr: f32, settings: &Settings) -> Value {
    let mut request = json!({
        "oversizeFactor": settings.oversize_factor,
        "undersizeFactor": settings.undersize_factor,
        "checkLcp": settings.check_lcp,
    });
    if dpr.is_finite() && dpr > 0.0 {
        request["dpr"] = json!(f64::from(dpr));
    }
    request
}

/// The script `evaluate` runs: the collector and its call with `request`.
#[must_use]
pub fn script(request: &Value) -> String {
    format!("(() => {{ {COLLECT_JS}\n; return ResponsiveImagesCollect.run({request}); }})()")
}

/// The images of the page `view` shows, measured at `dpr`.
///
/// # Errors
///
/// The host's failure (the view is gone, its engine has no `evaluate`), or
/// `plugins.error.bad_output` for an answer that is no list of images.
pub fn collect(
    host: &mut dyn PluginHost,
    view: ViewId,
    dpr: f32,
    settings: &Settings,
) -> Result<PageImages, Failure> {
    let answer = host.evaluate(view, &script(&request(dpr, settings)))?;
    parse(&answer)
}

/// The collector's answer read.
///
/// # Errors
///
/// `plugins.error.bad_output` for an answer that is no list of images.
pub fn parse(answer: &str) -> Result<PageImages, Failure> {
    // Only an object is a page: serde would also read a struct from an array (`[]`).
    serde_json::from_str::<serde_json::Value>(answer)
        .ok()
        .filter(serde_json::Value::is_object)
        .and_then(|value| serde_json::from_value::<PageImages>(value).ok())
        .map(PageImages::sanitized)
        .ok_or_else(|| {
            Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT).with_param("reason", "collect")
        })
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
