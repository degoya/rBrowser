//! The MCP tool `seo_social_preview` (the SEO panel calls it too): the cards of every platform
//! the settings name for a page (`url`, else the target view's), and each share image's real
//! format and size - as a `data:` URI while the answer stays below the panel's 64 KiB. Each
//! card's layout follows its platform's rule with the image's real size (RB-080-99).

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use scraper::Html;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::config::Config;
use crate::fetch::{self, MAX_HOPS};
use crate::page;
use crate::rules::short;
use crate::social::{self, Shape, base64, share};
use crate::tool::{bad_args, view_of};
use crate::url::{MAX_URL_CHARS, Url};

/// The most bytes an answer may have with its `data:` URIs: the panels window refuses more than
/// 64 KiB.
pub const ANSWER_BUDGET: usize = 60_000;
/// The most distinct share images of one page fetched (`og:image` and `twitter:image`).
const MAX_IMAGES: usize = 2;

/// The arguments of `seo_social_preview`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Args {
    url: Option<String>,
    /// `"all"` or a view id, as the host resolves MCP's `target`.
    target: Option<Value>,
}

fn bad(code: &str) -> Failure {
    Failure::coded(FailureKind::BadData, code)
}

/// The page to preview: `url`, else the view's.
fn page_url(host: &mut dyn PluginHost, args: &Args) -> Result<Url, Failure> {
    let text = match &args.url {
        Some(url) => Some(url.clone()),
        None => {
            let view = view_of(host, args.target.as_ref())?;
            match view {
                Some(view) => host
                    .views()
                    .ok()
                    .and_then(|views| views.into_iter().find(|info| info.id == view))
                    .map(|info| info.url),
                None => None,
            }
        }
    };
    let text = text.ok_or_else(|| bad("plugin.seo.errors.no_url"))?;
    Url::parse(&text)
        .ok_or_else(|| bad("plugin.seo.errors.bad_url").with_param("url", short(&text, 200)))
}

/// One share image as the answer lists it, its bytes kept for a `data:` URI, and its size for
/// the platforms' rules (`None`: not loaded, no image, or an SVG).
fn image_entry(host: &mut dyn PluginHost, url: &str) -> (Value, Option<String>, Option<Shape>) {
    let Some(parsed) = Url::parse(url) else {
        return (json!({ "url": url, "error": "url" }), None, None);
    };
    let loaded = share::load(host, &parsed);
    let sniffed = loaded.image.filter(|_| loaded.ok());
    let shape = sniffed
        .filter(|image| loaded.is_image() && image.format != social::image::Format::Svg)
        .and_then(|image| image.size)
        .map(|(width, height)| Shape {
            width,
            height,
            bytes: u64::try_from(loaded.body.len()).unwrap_or(u64::MAX),
        });
    let entry = json!({
        "url": url,
        "status": loaded.status,
        "error": loaded.failure,
        "format": sniffed.map(|image| image.format.mime()),
        "width": sniffed.and_then(|image| image.size).map(|(w, _)| w),
        "height": sniffed.and_then(|image| image.size).map(|(_, h)| h),
        "bytes": loaded.body.len(),
        "dataUri": null,
        "tooLarge": false,
    });
    // An SVG is never inlined: the platforms show none, and the panel needs no markup.
    let data_uri = sniffed
        .filter(|image| image.format != social::image::Format::Svg && loaded.is_image())
        .map(|image| {
            format!(
                "data:{};base64,{}",
                image.format.mime(),
                base64::encode(&loaded.body)
            )
        });
    (entry, data_uri, shape)
}

/// Runs `seo_social_preview` with `args` (JSON); answers JSON.
///
/// # Errors
///
/// The arguments are malformed (`plugin.seo.errors.bad_args`), there is no page
/// (`plugin.seo.errors.no_url`) or none that is `http(s)` (`plugin.seo.errors.bad_url`).
pub fn preview(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let value: Value = if args.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(args).map_err(|_| bad_args("json"))?
    };
    let parsed: Args = serde_json::from_value(value.clone()).map_err(|_| bad_args("fields"))?;
    if parsed
        .url
        .as_ref()
        .is_some_and(|url| url.len() > MAX_URL_CHARS)
    {
        return Err(bad_args("url"));
    }
    let config = Config::read(host, &value);
    let start = page_url(host, &parsed)?;
    let fetched = fetch::follow(host, &start, MAX_HOPS);
    let response = fetched.response.as_ref();
    let final_url = response
        .and_then(|response| Url::parse(&response.url))
        .unwrap_or_else(|| start.clone());
    let facts = response
        .filter(|response| response.status == 200 && response.is_html())
        .map(|response| page::facts(&Html::parse_document(&response.text())))
        .unwrap_or_default();
    let mut cards = social::cards(&facts, &final_url, &config.social_platforms);
    let mut urls: Vec<String> = Vec::new();
    for url in cards.iter().filter_map(|card| card.image.clone()) {
        if !urls.contains(&url) && urls.len() < MAX_IMAGES {
            urls.push(url);
        }
    }
    let loaded: Vec<(Value, Option<String>, Option<Shape>)> =
        urls.iter().map(|url| image_entry(host, url)).collect();
    for card in &mut cards {
        let Some(image) = card.image.clone() else {
            continue;
        };
        let shape = urls
            .iter()
            .position(|url| *url == image)
            .and_then(|at| loaded.get(at))
            .and_then(|(_, _, shape)| *shape);
        card.settle(shape);
    }
    let mut answer = json!({
        "url": start.to_string(),
        "finalUrl": final_url.to_string(),
        "status": response.map(|response| response.status),
        "domain": social::domain(&facts, &final_url),
        "dataVersion": social::data().version,
        "cards": cards,
        "images": loaded.iter().map(|(entry, _, _)| entry.clone()).collect::<Vec<_>>(),
    });
    let mut size = answer.to_string().len();
    for (index, (_, data_uri, _)) in loaded.into_iter().enumerate() {
        let Some(data_uri) = data_uri else { continue };
        let entry = &mut answer["images"][index];
        if size + data_uri.len() <= ANSWER_BUDGET {
            size += data_uri.len();
            entry["dataUri"] = Value::String(data_uri);
        } else {
            entry["tooLarge"] = Value::Bool(true);
        }
    }
    Ok(answer.to_string())
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
