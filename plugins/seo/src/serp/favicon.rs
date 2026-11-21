//! The favicon of the search result: the page's `<link rel="icon">` (an SVG one left out: the
//! panel takes no markup), else its `apple-touch-icon`, else the site's `/favicon.ico` - fetched
//! once through the host as a share image is (`social::share`), sniffed as PNG, JPEG, GIF, WebP
//! or ICO and inlined as a `data:` URI up to [`MAX_BYTES`] while the answer has room.

use rb_plugin_api::PluginHost;
use scraper::Html;
use serde_json::{Value, json};

use crate::page::sel;
use crate::social::image::{self, Format};
use crate::social::{base64, share};
use crate::url::Url;

/// The largest favicon inlined; a larger one is described, not shown (`tooLarge`).
pub const MAX_BYTES: usize = 100 * 1024;
/// The MIME type of an ICO file.
const ICO: &str = "image/x-icon";

fn has_token(rel: &str, token: &str) -> bool {
    rel.split_ascii_whitespace()
        .any(|part| part.eq_ignore_ascii_case(token))
}

/// Whether an icon link names an SVG by its `type` or its file's extension.
fn is_svg(kind: Option<&str>, href: &str) -> bool {
    let path = href.split(['?', '#']).next().unwrap_or_default();
    kind.is_some_and(|kind| kind.to_ascii_lowercase().contains("svg"))
        || path.to_ascii_lowercase().ends_with(".svg")
}

/// The favicon of the page at `page` with `html` (`None`: not loaded) and where it came from:
/// `icon`, `apple-touch-icon` or `default` (`/favicon.ico`).
#[must_use]
pub fn pick(html: Option<&Html>, page: &Url) -> (&'static str, Url) {
    let mut icons = Vec::new();
    let mut touch = Vec::new();
    let links = sel("link[rel][href]");
    for link in html.into_iter().flat_map(|html| html.select(&links)) {
        let element = link.value();
        let rel = element.attr("rel").unwrap_or_default();
        let href = element.attr("href").unwrap_or_default();
        if has_token(rel, "icon") && !is_svg(element.attr("type"), href) {
            icons.push(href);
        } else if has_token(rel, "apple-touch-icon")
            || has_token(rel, "apple-touch-icon-precomposed")
        {
            touch.push(href);
        }
    }
    let joined = |source: &'static str, hrefs: Vec<&str>| {
        hrefs
            .into_iter()
            .find_map(|href| page.join(href))
            .map(|url| (source, url))
    };
    joined("icon", icons)
        .or_else(|| joined("apple-touch-icon", touch))
        .or_else(|| page.join("/favicon.ico").map(|url| ("default", url)))
        .unwrap_or_else(|| ("default", page.clone()))
}

/// The MIME type and size of a favicon's bytes: a raster share image format or ICO (the size of
/// its first entry); `None` for an SVG and anything else.
#[must_use]
pub fn sniff(bytes: &[u8]) -> Option<(&'static str, Option<(u32, u32)>)> {
    if let Some(sniffed) = image::sniff(bytes) {
        return (sniffed.format != Format::Svg).then_some((sniffed.format.mime(), sniffed.size));
    }
    let count = bytes
        .get(4..6)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
    if bytes.starts_with(&[0, 0, 1, 0]) && count.is_some_and(|count| count > 0) {
        let side = |at: usize| {
            bytes
                .get(at)
                .map(|&side| if side == 0 { 256 } else { u32::from(side) })
        };
        return Some((ICO, side(6).zip(side(7))));
    }
    None
}

/// The favicon at `url` (found as `source`) fetched: its entry in the answer, and its `data:`
/// URI when it is an image of a sniffed format within [`MAX_BYTES`].
pub fn load(host: &mut dyn PluginHost, source: &str, url: &Url) -> (Value, Option<String>) {
    let loaded = share::load(host, url);
    let typed = loaded
        .content_type
        .as_deref()
        .is_none_or(|kind| kind.trim().to_ascii_lowercase().starts_with("image/"));
    let sniffed = sniff(&loaded.body).filter(|_| loaded.ok() && typed);
    let too_large = sniffed.is_some() && loaded.body.len() > MAX_BYTES;
    let entry = json!({
        "url": url.to_string(),
        "source": source,
        "status": loaded.status,
        "error": loaded.failure,
        "format": sniffed.map(|(mime, _)| mime),
        "width": sniffed.and_then(|(_, size)| size).map(|(width, _)| width),
        "height": sniffed.and_then(|(_, size)| size).map(|(_, height)| height),
        "bytes": loaded.body.len(),
        "dataUri": null,
        "tooLarge": too_large,
    });
    let data_uri = sniffed
        .filter(|_| !too_large)
        .map(|(mime, _)| format!("data:{mime};base64,{}", base64::encode(&loaded.body)));
    (entry, data_uri)
}

#[cfg(test)]
#[path = "favicon_tests.rs"]
mod tests;
