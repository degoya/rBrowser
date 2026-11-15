//! The font inventory over the views (`fonts_list`, the panel, the export): each family with its
//! source, formats, faces (status per view, file, size, preload), where it is used per view and
//! what Chromium really drew; the font files; the views and why some gave nothing.

use serde::Serialize;
use serde_json::{Value, json};

use crate::page::Face;
use crate::rendered::{self, Verdict};
use crate::run::{Collected, Rendered, ViewFonts};
use crate::source::{self, Provider};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ViewState {
    pub view: u32,
    pub state: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceEntry {
    pub weight: String,
    pub style: String,
    pub display: String,
    pub unicode_range: String,
    pub status: Vec<ViewState>,
    /// The first file, if the rule names one.
    pub src: Option<String>,
    pub format: String,
    pub kb: Option<f64>,
    pub preloaded: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UsageEntry {
    pub view: u32,
    pub elements: u64,
    pub weights: Vec<u64>,
    pub styles: Vec<String>,
    pub sizes: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RenderedEntry {
    pub view: u32,
    /// `ok`, `fallback`, `glyphs`, `unknown` or `unsupported` (no CDP: Firefox, WebKit).
    pub state: String,
    /// The fonts drawn instead.
    #[serde(rename = "as")]
    pub instead: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Family {
    pub family: String,
    /// `google`, `adobe`, `own`, `external`, `inline`, `system`, `generic`, `unknown`.
    pub provider: String,
    pub host: String,
    pub web_font: bool,
    pub formats: Vec<String>,
    pub variable: bool,
    pub faces: Vec<FaceEntry>,
    pub file_kb: Option<f64>,
    pub usage: Vec<UsageEntry>,
    pub rendered: Vec<RenderedEntry>,
}

fn kb(bytes: u64) -> f64 {
    (bytes as f64 / 1024.0 * 10.0).round() / 10.0
}

fn find<'a>(families: &'a mut Vec<Family>, name: &str) -> &'a mut Family {
    let index = match families
        .iter()
        .position(|f| f.family.eq_ignore_ascii_case(name))
    {
        Some(index) => index,
        None => {
            let generic = rendered::generic(name);
            families.push(Family {
                family: name.to_owned(),
                provider: if generic {
                    "generic"
                } else {
                    Provider::System.as_str()
                }
                .to_owned(),
                host: String::new(),
                web_font: false,
                formats: Vec::new(),
                variable: false,
                faces: Vec::new(),
                file_kb: None,
                usage: Vec::new(),
                rendered: Vec::new(),
            });
            families.len() - 1
        }
    };
    &mut families[index]
}

fn add_face(family: &mut Family, view: &ViewFonts, face: &Face) {
    let fonts = &view.fonts;
    let (provider, host) = source::of_face(face, &fonts.url);
    if !family.web_font || family.provider == Provider::Unknown.as_str() {
        family.provider = provider.as_str().to_owned();
        family.host = host;
    }
    family.web_font = true;
    family.variable |= face.variable;
    for format in source::formats(face) {
        if !family.formats.contains(&format) {
            family.formats.push(format);
        }
    }
    let src = source::urls(face).next().map(str::to_owned);
    let size = src
        .as_deref()
        .and_then(|url| fonts.resources.iter().find(|file| file.url == url))
        .map(|file| file.size)
        .filter(|size| *size > 0);
    let status = ViewState {
        view: view.view.id,
        state: face.status.clone(),
    };
    let same = |entry: &&mut FaceEntry| {
        entry.weight == face.weight
            && entry.style == face.style
            && entry.unicode_range == face.unicode_range
    };
    if let Some(entry) = family.faces.iter_mut().find(|entry| same(entry)) {
        entry.status.push(status);
        entry.kb = entry.kb.or(size.map(kb));
        return;
    }
    family.faces.push(FaceEntry {
        weight: face.weight.clone(),
        style: face.style.clone(),
        display: face.display.clone(),
        unicode_range: face.unicode_range.clone(),
        status: vec![status],
        preloaded: src.as_deref().is_some_and(|url| fonts.preloaded(url)),
        format: source::formats(face).first().cloned().unwrap_or_default(),
        src,
        kb: size.map(kb),
    });
}

fn add_rendered(families: &mut Vec<Family>, view: &ViewFonts) {
    let results = match &view.rendered {
        Rendered::Read(results) => results,
        Rendered::Unsupported => {
            for family in families
                .iter_mut()
                .filter(|f| f.usage.iter().any(|u| u.view == view.view.id))
            {
                family.rendered.push(RenderedEntry {
                    view: view.view.id,
                    state: "unsupported".to_owned(),
                    instead: Vec::new(),
                });
            }
            return;
        }
        Rendered::Skipped => return,
    };
    for result in results {
        let family = find(families, &result.family);
        let (state, instead) = match &result.verdict {
            Verdict::Fallback { rendered } => ("fallback", Some(rendered)),
            Verdict::Glyphs { fallback, .. } => ("glyphs", Some(fallback)),
            Verdict::Match => ("ok", None),
            Verdict::Unknown => ("unknown", None),
        };
        let rank = |state: &str| {
            ["unknown", "ok", "glyphs", "fallback"]
                .iter()
                .position(|s| *s == state)
        };
        let entry = match family
            .rendered
            .iter_mut()
            .position(|e| e.view == view.view.id)
        {
            Some(index) => &mut family.rendered[index],
            None => {
                family.rendered.push(RenderedEntry {
                    view: view.view.id,
                    state: state.to_owned(),
                    instead: Vec::new(),
                });
                let last = family.rendered.len() - 1;
                &mut family.rendered[last]
            }
        };
        if rank(state) > rank(&entry.state) {
            entry.state = state.to_owned();
        }
        if let Some(name) = instead
            && !entry.instead.contains(name)
        {
            entry.instead.push(name.clone());
        }
    }
}

/// Every family of the views: the web fonts first (in the order the pages declare them), then
/// the installed and generic ones the text asks for.
#[must_use]
pub fn families(collected: &Collected) -> Vec<Family> {
    let mut families: Vec<Family> = Vec::new();
    for view in &collected.views {
        for face in &view.fonts.faces {
            let family = find(&mut families, &face.family);
            add_face(family, view, face);
        }
    }
    for view in &collected.views {
        for usage in &view.fonts.usages {
            if usage.family.is_empty() {
                continue;
            }
            let family = find(&mut families, &usage.family);
            let entry = match family.usage.iter().position(|u| u.view == view.view.id) {
                Some(index) => &mut family.usage[index],
                None => {
                    family.usage.push(UsageEntry {
                        view: view.view.id,
                        elements: 0,
                        weights: Vec::new(),
                        styles: Vec::new(),
                        sizes: Vec::new(),
                    });
                    let last = family.usage.len() - 1;
                    &mut family.usage[last]
                }
            };
            entry.elements = entry.elements.saturating_add(usage.count);
            if !entry.weights.contains(&usage.weight) {
                entry.weights.push(usage.weight);
            }
            if !entry.styles.contains(&usage.style) {
                entry.styles.push(usage.style.clone());
            }
            for size in usage.sizes() {
                if !entry.sizes.contains(&size) && entry.sizes.len() < 16 {
                    entry.sizes.push(size);
                }
            }
        }
        add_rendered(&mut families, view);
    }
    for family in &mut families {
        let total: f64 = family.faces.iter().filter_map(|face| face.kb).sum();
        family.file_kb = (total > 0.0).then(|| (total * 10.0).round() / 10.0);
        for usage in &mut family.usage {
            usage.weights.sort_unstable();
            usage.sizes.sort_by(f64::total_cmp);
        }
    }
    families
}

/// The font files the views loaded, each once.
#[must_use]
pub fn files(collected: &Collected) -> Vec<Value> {
    let mut seen: Vec<&str> = Vec::new();
    let mut list = Vec::new();
    for view in &collected.views {
        for file in &view.fonts.resources {
            if seen.contains(&file.url.as_str()) {
                continue;
            }
            seen.push(&file.url);
            let (provider, host) = source::classify(&file.url, &view.fonts.url);
            list.push(json!({
                "url": file.url,
                "kb": (file.size > 0).then(|| kb(file.size)),
                "provider": provider.as_str(),
                "host": host,
            }));
        }
    }
    list
}

/// The views of a call: each with its engine, width, how far it was read and why not.
#[must_use]
pub fn views(collected: &Collected) -> Vec<Value> {
    let mut list: Vec<Value> = collected
        .views
        .iter()
        .map(|view| {
            json!({
                "id": view.view.id, "label": view.view.label, "breakpoint": view.view.breakpoint,
                "engine": view.view.engine, "width": view.width(), "url": view.fonts.url,
                "elements": view.fonts.elements, "truncated": view.fonts.truncated,
                "rendered": view.rendered.as_str(),
            })
        })
        .collect();
    for (view, code) in &collected.failures {
        list.push(json!({
            "id": view.id, "label": view.label, "breakpoint": view.breakpoint,
            "engine": view.engine, "width": view.width, "url": view.url,
            "error": code, "rendered": "unsupported",
        }));
    }
    list
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
