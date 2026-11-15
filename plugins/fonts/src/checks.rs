//! The findings of one view: privacy, performance, readability and rendered vs declared
//! (`rules.rs` names the rules).

use rb_plugin_api::Finding;

use crate::page::{Face, PageFonts};
use crate::rendered::Verdict;
use crate::rules::{self, At, Sink, shown};
use crate::run::{SampleResult, ViewFonts};
use crate::settings::Settings;
use crate::source::{self, Provider};

/// Views up to this wide are phones and tablets: an input under `minInputPx` makes iOS zoom.
pub const MAX_ZOOM_WIDTH: f64 = 1024.0;

/// `Inter 600 normal`.
#[must_use]
pub fn face_name(face: &Face) -> String {
    format!("{} {} {}", face.family, face.weight, face.style)
}

fn face_params(face: &Face) -> Vec<(&'static str, String)> {
    vec![
        ("family", face.family.clone()),
        ("weight", face.weight.clone()),
        ("style", face.style.clone()),
    ]
}

/// Whether a face of weight `face` (`400`, or a range `100 900`) serves text of weight `text`.
fn weight_serves(face: &str, text: u64) -> bool {
    let bounds: Vec<u64> = face
        .split_whitespace()
        .filter_map(|part| part.parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n >= 0.0)
        .map(|n| n.round() as u64)
        .collect();
    match bounds.as_slice() {
        [one] => *one == text,
        [low, high, ..] => (*low..=*high).contains(&text),
        _ => false,
    }
}

fn used_above_fold(fonts: &PageFonts, face: &Face) -> bool {
    fonts.usages.iter().any(|usage| {
        usage.above_fold
            && usage.family.eq_ignore_ascii_case(&face.family)
            && weight_serves(&face.weight, usage.weight)
            && usage.style == face.style
    })
}

fn privacy(sink: &mut Sink<'_>, fonts: &PageFonts, settings: &Settings) {
    let mut reported: Vec<(String, String)> = Vec::new();
    let mut hosts: Vec<String> = Vec::new();
    for face in &fonts.faces {
        let (provider, host) = source::of_face(face, &fonts.url);
        // A readable face never needed was never requested; a face of an unreadable sheet was
        // (the sheet itself came from that host).
        let requested = face.status != "unloaded" || !face.readable;
        if !provider.third_party() || host.is_empty() || !requested || settings.host_allowed(&host)
        {
            continue;
        }
        let entry = (face.family.to_ascii_lowercase(), host.clone());
        if reported.contains(&entry) {
            continue;
        }
        reported.push(entry);
        hosts.push(host.clone());
        sink.emit(
            rules::EXTERNAL_HOST,
            "external-host",
            &At::Named(format!("@font-host {host} {}", face.family)),
            &[
                ("family", face.family.clone()),
                ("host", host.clone()),
                ("provider", source::provider_name(provider, &host)),
            ],
        );
    }
    let sheets = fonts.sheets.iter().filter_map(|sheet| sheet.as_str());
    let files = fonts.resources.iter().map(|file| file.url.as_str());
    for url in sheets.chain(files) {
        let (provider, host) = source::classify(url, &fonts.url);
        let font_service = matches!(provider, Provider::Google | Provider::Adobe);
        let file = fonts.resources.iter().any(|file| file.url == url);
        if !(font_service || (file && provider.third_party()))
            || host.is_empty()
            || hosts.contains(&host)
            || settings.host_allowed(&host)
        {
            continue;
        }
        hosts.push(host.clone());
        sink.emit(
            rules::EXTERNAL_HOST,
            "external-host-file",
            &At::Named(format!("@font-host {host}")),
            &[
                ("host", host.clone()),
                ("provider", source::provider_name(provider, &host)),
            ],
        );
    }
}

fn faces(sink: &mut Sink<'_>, fonts: &PageFonts) {
    for face in &fonts.faces {
        let urls: Vec<&str> = source::urls(face).collect();
        let web = !urls.is_empty() || !face.readable;
        if !web {
            continue;
        }
        let at = At::Named(format!("@font-face {}", face_name(face)));
        let params = face_params(face);
        if matches!(face.display.as_str(), "auto" | "") && face.status != "unloaded" {
            sink.emit(
                rules::FONT_DISPLAY_MISSING,
                rules::FONT_DISPLAY_MISSING,
                &at,
                &params,
            );
        }
        let formats = source::formats(face);
        let woff2 = formats.iter().any(|format| format == "woff2")
            || urls.iter().any(|url| {
                url.split(['?', '#'])
                    .next()
                    .is_some_and(|path| path.ends_with(".woff2"))
            });
        if !urls.is_empty() && !woff2 {
            let mut params = params.clone();
            params.push(("format", formats.join(", ")));
            sink.emit(
                rules::FORMAT_NOT_WOFF2,
                rules::FORMAT_NOT_WOFF2,
                &at,
                &params,
            );
        }
        if face.status == "unloaded" {
            sink.emit(rules::FACE_UNUSED, rules::FACE_UNUSED, &at, &params);
        }
        if face.status == "loaded"
            && !urls.is_empty()
            && used_above_fold(fonts, face)
            && !urls.iter().any(|url| fonts.preloaded(url))
        {
            let url = urls
                .iter()
                .find(|url| url.contains(".woff2"))
                .or_else(|| urls.first())
                .map_or_else(String::new, |url| (*url).to_owned());
            let mut params = params.clone();
            params.push(("url", url));
            sink.emit(rules::PRELOAD_MISSING, rules::PRELOAD_MISSING, &at, &params);
        }
    }
}

fn weights_and_files(sink: &mut Sink<'_>, fonts: &PageFonts, settings: &Settings) {
    let mut families: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for face in fonts
        .faces
        .iter()
        .filter(|face| matches!(face.status.as_str(), "loaded" | "loading"))
    {
        let cut = (face.weight.clone(), face.style.clone());
        match families
            .iter_mut()
            .find(|(family, _)| family.eq_ignore_ascii_case(&face.family))
        {
            Some((_, cuts)) if !cuts.contains(&cut) => cuts.push(cut),
            Some(_) => {}
            None => families.push((face.family.clone(), vec![cut])),
        }
    }
    let max = usize::try_from(settings.max_weights).unwrap_or(usize::MAX);
    for (family, cuts) in families.iter().filter(|(_, cuts)| cuts.len() > max) {
        sink.emit(
            rules::TOO_MANY_WEIGHTS,
            rules::TOO_MANY_WEIGHTS,
            &At::Named(format!("@font-family {family}")),
            &[
                ("family", family.clone()),
                ("count", cuts.len().to_string()),
                ("max", settings.max_weights.to_string()),
            ],
        );
    }
    let limit = settings.max_file_kb.saturating_mul(1024);
    for file in fonts.resources.iter().filter(|file| file.size > limit) {
        sink.emit(
            rules::LARGE_FILE,
            rules::LARGE_FILE,
            &At::Named(format!("@font-file {}", file.url)),
            &[
                ("url", file.url.clone()),
                ("kb", shown(file.size as f64 / 1024.0)),
                ("max", settings.max_file_kb.to_string()),
            ],
        );
    }
}

fn readability(sink: &mut Sink<'_>, fonts: &PageFonts, width: f64, settings: &Settings) {
    for text in fonts
        .small_text
        .iter()
        .filter(|c| c.size > 0.0 && c.size < settings.min_text_px)
    {
        sink.emit(
            rules::TEXT_TOO_SMALL,
            rules::TEXT_TOO_SMALL,
            &At::Element(&text.at),
            &[
                ("size", shown(text.size)),
                ("min", shown(settings.min_text_px)),
                ("text", text.text.clone()),
            ],
        );
    }
    if width <= MAX_ZOOM_WIDTH {
        for input in fonts
            .small_inputs
            .iter()
            .filter(|c| c.size > 0.0 && c.size < settings.min_input_px)
        {
            sink.emit(
                rules::INPUT_TOO_SMALL,
                rules::INPUT_TOO_SMALL,
                &At::Element(&input.at),
                &[
                    ("size", shown(input.size)),
                    ("min", shown(settings.min_input_px)),
                ],
            );
        }
    }
    for line in fonts
        .tight_lines
        .iter()
        .filter(|c| c.line_height < settings.min_line_height)
    {
        sink.emit(
            rules::LINE_HEIGHT_TIGHT,
            rules::LINE_HEIGHT_TIGHT,
            &At::Element(&line.at),
            &[
                ("ratio", shown(line.line_height)),
                ("min", shown(settings.min_line_height)),
                ("text", line.text.clone()),
            ],
        );
    }
}

fn rendered(sink: &mut Sink<'_>, results: &[SampleResult]) {
    for result in results {
        match &result.verdict {
            Verdict::Fallback { rendered } => sink.emit(
                rules::RENDERED_FALLBACK,
                rules::RENDERED_FALLBACK,
                &At::Element(&result.at),
                &[
                    ("declared", result.family.clone()),
                    ("rendered", rendered.clone()),
                    ("text", result.text.clone()),
                ],
            ),
            Verdict::Glyphs { fallback, glyphs } => sink.emit(
                rules::GLYPH_FALLBACK,
                rules::GLYPH_FALLBACK,
                &At::Element(&result.at),
                &[
                    ("family", result.family.clone()),
                    ("fallback", fallback.clone()),
                    ("glyphs", glyphs.to_string()),
                    ("chars", result.non_ascii.clone()),
                ],
            ),
            Verdict::Match | Verdict::Unknown => {}
        }
    }
}

/// The findings of one view (the breakpoint rule is `fluid.rs`'s, over every view).
#[must_use]
pub fn view(enabled: &[String], view: &ViewFonts, settings: &Settings) -> Vec<Finding> {
    let fonts = &view.fonts;
    let url = if fonts.url.is_empty() {
        view.view.url.as_str()
    } else {
        fonts.url.as_str()
    };
    let mut sink = Sink::new(enabled, view.view.id, url);
    privacy(&mut sink, fonts, settings);
    faces(&mut sink, fonts);
    weights_and_files(&mut sink, fonts, settings);
    readability(&mut sink, fonts, view.width(), settings);
    if let Some(results) = view.rendered.results() {
        rendered(&mut sink, results);
    }
    sink.findings
}

#[cfg(test)]
#[path = "checks_tests.rs"]
mod tests;
