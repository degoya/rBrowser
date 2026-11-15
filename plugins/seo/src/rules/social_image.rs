//! RB-080-11, the share image of the page (`og:image`, resolved against the page even when it is
//! written relative): `social.image_unreachable` (no 200), `social.image_type` (no image, or an
//! SVG, which no platform shows), `social.image_small` (under `seo.social.imageMinWidth` x
//! `imageMinHeight`, 1200 x 630) and `social.image_ratio` (more than a tenth off 1.91:1).
//! RB-080-99: `social.card_small`, a platform of `seo.social.platforms` whose rule shows the image
//! as a small card although the page asks for a large one (`twitter:card` summary_large_image),
//! with the reason. One fetch through the host; only for the start page of a run.

use rb_plugin_api::PluginHost;

use crate::audit::Page;
use crate::config::Config;
use crate::page::Facts;
use crate::rules::{Sink, short};
use crate::social::image::Format;
use crate::social::variant::{self, LARGE_CARD, Layout, Rule, Shape};
use crate::social::{data, share};

/// The aspect ratio the platforms crop a large card to.
pub const RATIO: f64 = 1.91;
/// How far (a fraction of [`RATIO`]) an image's ratio may stray before it is a finding.
pub const RATIO_TOLERANCE: f64 = 0.1;

const RULES: [&str; 5] = [
    "social.image_unreachable",
    "social.image_type",
    "social.image_small",
    "social.image_ratio",
    "social.card_small",
];

/// `social.card_small`: every chosen platform with a size rule that shows `shape` small while the
/// page's `twitter:card` asks for the large card; the message names the reason
/// (`social.card_small_<reason>`).
fn card_small(facts: &Facts, config: &Config, shape: Shape, sink: &mut Sink<'_>) {
    let wants_large = facts
        .social
        .get("twitter:card")
        .is_some_and(|card| card.trim().eq_ignore_ascii_case(LARGE_CARD));
    if !wants_large || !sink.wants("social.card_small") {
        return;
    }
    for platform in data().platforms {
        if !config.social_platforms.contains(&platform.id)
            || !matches!(platform.rule, Rule::Size(_))
        {
            continue;
        }
        let chosen = variant::choose(&platform.rule, Some(LARGE_CARD), true, Some(shape));
        if chosen.layout != Layout::Small {
            continue;
        }
        let mut params = vec![("platform", platform.name.clone())];
        for (key, value) in &chosen.reason.params {
            params.push((key.as_str(), value.clone()));
        }
        let message = format!("social.card_small_{}", chosen.reason.code);
        sink.emit("social.card_small", &message, None, &params);
    }
}

pub fn check(host: &mut dyn PluginHost, page: &Page<'_>, sink: &mut Sink<'_>) {
    if !RULES.iter().any(|rule| sink.wants(rule)) {
        return;
    }
    let Some((_, facts)) = page.primary() else {
        return;
    };
    let Some(href) = facts
        .social
        .get("og:image")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Some(url) = page.url.join(href) else {
        return;
    };
    let loaded = share::load(host, &url);
    let shown = short(&url.to_string(), 200);
    if !loaded.ok() {
        let status = loaded
            .status
            .map(|status| status.to_string())
            .or(loaded.failure.clone())
            .unwrap_or_else(|| "-".to_owned());
        sink.emit(
            "social.image_unreachable",
            "social.image_unreachable",
            None,
            &[("url", shown), ("status", status)],
        );
        return;
    }
    let kind = loaded
        .content_type
        .clone()
        .unwrap_or_else(|| "-".to_owned());
    let Some(image) = loaded.image.filter(|_| loaded.is_image()) else {
        sink.emit(
            "social.image_type",
            "social.image_type",
            None,
            &[("url", shown), ("type", short(&kind, 100))],
        );
        return;
    };
    let Some((width, height)) = image.size.filter(|_| image.format != Format::Svg) else {
        sink.emit(
            "social.image_type",
            "social.image_svg",
            None,
            &[("url", shown)],
        );
        return;
    };
    let config = page.config;
    let size = [("width", width.to_string()), ("height", height.to_string())];
    if width < config.image_min_width || height < config.image_min_height {
        let mut params = vec![("url", shown.clone())];
        params.extend(size.iter().cloned());
        params.push(("min_width", config.image_min_width.to_string()));
        params.push(("min_height", config.image_min_height.to_string()));
        sink.emit("social.image_small", "social.image_small", None, &params);
    }
    let shape = Shape {
        width,
        height,
        bytes: u64::try_from(loaded.body.len()).unwrap_or(u64::MAX),
    };
    card_small(facts, config, shape, sink);
    if height > 0 {
        let ratio = f64::from(width) / f64::from(height);
        if ((ratio - RATIO) / RATIO).abs() > RATIO_TOLERANCE {
            let mut params = vec![("url", shown)];
            params.extend(size.iter().cloned());
            params.push(("ratio", format!("{ratio:.2}")));
            sink.emit("social.image_ratio", "social.image_ratio", None, &params);
        }
    }
}

#[cfg(test)]
#[path = "social_image_tests.rs"]
mod tests;
