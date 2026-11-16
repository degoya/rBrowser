//! Parity of the views (RB-060-09): with mobile-first indexing, what only the desktop page has
//! does not count. Every Chromium view that shows the page gives its rendered DOM; title, h1,
//! canonical, hreflang, the JSON-LD types and the visible main text of each are held to the
//! reference view's (`seo.parity.reference`, the narrowest view by default). A difference is a
//! finding on the differing view, with both values and both breakpoints.

use std::collections::BTreeSet;

use rb_plugin_api::{PluginHost, ViewId, ViewInfo};
use serde::Serialize;
use serde_json::Value;

use crate::config::{Config, SMALLEST};
use crate::hreflang::sources::html_alternates;
use crate::page;
use crate::rendered::Rendered;
use crate::rules::{Sink, short};
use crate::url::Url;

/// What a value is shown as when a view has none.
const NONE: &str = "-";

/// The compared values of one view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParityView {
    pub view: ViewId,
    pub breakpoint: String,
    pub width: u32,
    pub title: String,
    pub h1: String,
    pub canonical: String,
    pub hreflang: String,
    pub structured_data: String,
    /// The main text's similarity to the reference view's, in percent; none without a text.
    pub text_similarity: Option<u32>,
    #[serde(skip)]
    pub(crate) text: Option<String>,
}

/// The views compared and the reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ParityReport {
    pub reference: ViewId,
    pub views: Vec<ParityView>,
}

/// The JSON-LD `@type`s of a document, sorted, each once; a block that is no JSON is skipped.
fn types(blocks: &[page::JsonLd]) -> String {
    fn walk(value: &Value, out: &mut BTreeSet<String>) {
        match value {
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            Value::Object(map) => {
                match map.get("@type") {
                    Some(Value::String(kind)) => {
                        out.insert(kind.clone());
                    }
                    Some(Value::Array(kinds)) => {
                        out.extend(kinds.iter().filter_map(Value::as_str).map(str::to_owned));
                    }
                    _ => {}
                }
                if let Some(graph) = map.get("@graph") {
                    walk(graph, out);
                }
            }
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    for block in blocks {
        if let Ok(value) = serde_json::from_str::<Value>(&block.text) {
            walk(&value, &mut out);
        }
    }
    out.into_iter().collect::<Vec<_>>().join(", ")
}

fn or_none(text: String) -> String {
    if text.is_empty() {
        NONE.to_owned()
    } else {
        short(&text, 300)
    }
}

fn values(info: &ViewInfo, rendered: &Rendered, url: &Url) -> ParityView {
    let facts = page::facts(&rendered.html);
    let base = Url::parse(&rendered.url).unwrap_or_else(|| url.clone());
    let h1: Vec<&str> = facts
        .headings
        .iter()
        .filter(|heading| heading.level == 1)
        .map(|heading| heading.text.as_str())
        .collect();
    let mut hreflang: Vec<String> = html_alternates(&rendered.html, &base)
        .into_iter()
        .map(|alternate| {
            let target = alternate.url.map_or(alternate.href, |url| url.to_string());
            format!("{}={target}", alternate.code.to_ascii_lowercase())
        })
        .collect();
    hreflang.sort();
    let breakpoint = [&info.breakpoint, &info.label]
        .into_iter()
        .find(|name| !name.is_empty())
        .map(|name| (*name).clone())
        .unwrap_or_else(|| format!("view {}", info.id));
    ParityView {
        view: info.id,
        breakpoint,
        width: info.width,
        title: or_none(facts.titles.first().cloned().unwrap_or_default()),
        h1: or_none(h1.join(" | ")),
        canonical: or_none(
            facts
                .canonicals
                .first()
                .and_then(|href| base.join(href))
                .map(|url| url.to_string())
                .unwrap_or_default(),
        ),
        hreflang: or_none(hreflang.join(", ")),
        structured_data: or_none(types(&facts.json_ld)),
        text_similarity: None,
        text: rendered.text.clone(),
    }
}

fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The Jaccard similarity of the words of `a` and `b` (1 for two empty texts).
#[must_use]
pub fn similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (words(a), words(b));
    let union = a.union(&b).count();
    if union == 0 {
        return 1.0;
    }
    a.intersection(&b).count() as f64 / union as f64
}

fn reference(config: &Config, views: &[ParityView]) -> usize {
    let narrowest = views
        .iter()
        .enumerate()
        .min_by_key(|(_, view)| view.width)
        .map_or(0, |(index, _)| index);
    if config.parity_reference.eq_ignore_ascii_case(SMALLEST) {
        return narrowest;
    }
    views
        .iter()
        .position(|view| {
            view.breakpoint
                .eq_ignore_ascii_case(&config.parity_reference)
        })
        .unwrap_or(narrowest)
}

/// Compares the views showing `url`; `None` (logged) with fewer than two.
pub fn run(
    host: &mut dyn PluginHost,
    config: &Config,
    url: &Url,
    sink: &mut Sink<'_>,
) -> Option<ParityReport> {
    let infos = host.views().unwrap_or_default();
    let mut views = Vec::new();
    for info in infos.iter().filter(|info| {
        info.engine == "chromium" && Url::parse(&info.url).is_some_and(|shown| shown == *url)
    }) {
        match Rendered::collect(host, info.id) {
            Ok(rendered) => views.push(values(info, &rendered, url)),
            Err(failure) => host.log(
                "warn",
                &format!(
                    "seo: parity skips view {}: {}",
                    info.id,
                    failure.code().unwrap_or("unknown")
                ),
            ),
        }
    }
    if views.len() < 2 {
        host.log("info", "seo: parity needs two Chromium views of the page");
        return None;
    }
    let index = reference(config, &views);
    let base = views[index].clone();
    let page_url = url.to_string();
    for view in &mut views {
        if view.view == base.view {
            continue;
        }
        sink.page(&page_url, Some(view.view));
        let fields = [
            ("parity.title", "title", &view.title, &base.title),
            ("parity.h1", "h1", &view.h1, &base.h1),
            (
                "parity.canonical",
                "canonical",
                &view.canonical,
                &base.canonical,
            ),
            (
                "parity.hreflang",
                "hreflang",
                &view.hreflang,
                &base.hreflang,
            ),
            (
                "parity.structured_data",
                "structured data",
                &view.structured_data,
                &base.structured_data,
            ),
        ];
        for (rule, field, value, other) in fields {
            if value != other {
                sink.emit(
                    rule,
                    "parity.differs",
                    None,
                    &[
                        ("field", field.to_owned()),
                        ("breakpoint", view.breakpoint.clone()),
                        ("reference", base.breakpoint.clone()),
                        ("value", value.clone()),
                        ("reference_value", other.clone()),
                    ],
                );
            }
        }
        if let (Some(text), Some(reference_text)) = (&view.text, &base.text) {
            let ratio = similarity(text, reference_text);
            let percent = (ratio * 100.0).round() as u32;
            view.text_similarity = Some(percent);
            if ratio < config.text_similarity {
                sink.emit(
                    "parity.text",
                    "parity.text",
                    None,
                    &[
                        ("breakpoint", view.breakpoint.clone()),
                        ("reference", base.breakpoint.clone()),
                        ("similarity", percent.to_string()),
                        ("min", format!("{:.0}", config.text_similarity * 100.0)),
                        ("value", short(text, 200)),
                        ("reference_value", short(reference_text, 200)),
                    ],
                );
            }
        }
    }
    Some(ParityReport {
        reference: base.view,
        views,
    })
}

#[cfg(test)]
#[path = "parity_tests.rs"]
mod tests;
