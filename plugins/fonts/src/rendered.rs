//! Rendered vs declared (RB-070-13, Chromium only): the fonts Chromium really drew an element's
//! text with (`CSS.getPlatformFontsForNode`), compared with the first family its `font-family`
//! asks for - a fallback (`Inter` declared, `Arial` drawn) and glyphs from another font (an
//! ASCII-only subset whose umlauts come from the fallback).
//!
//! **The session is shared** with the host, the sync agent, peek and every other plugin (review
//! findings L5/L6 of Charge 6): `DOM.getDocument` drops every node id handed out before, and
//! `CSS.enable` would stay on for everyone. So, as peek does (`plugins/peek/src/node.rs`), the
//! document is named by the accessibility tree's root and pushed into the document someone
//! already requested; `DOM.getDocument` only when nobody has - then nobody holds a node id it
//! could drop - and no domain is switched on. Node ids are never kept across calls; a step that
//! fails because a node id went stale (another client's `DOM.getDocument` in between) resolves the
//! document once more and retries once.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde_json::{Value, json};

use crate::page::{Face, cut};

/// The most fonts kept per element.
pub const MAX_FONTS: usize = 16;
/// The longest family name kept, in characters.
const MAX_NAME: usize = 120;
/// The generic families and system keywords: nothing to compare.
const GENERIC: &[&str] = &[
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "math",
    "emoji",
    "fangsong",
    "-apple-system",
    "blinkmacsystemfont",
    "inherit",
    "initial",
    "unset",
];

/// A font an element's text was drawn with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedFont {
    pub family: String,
    /// A web font, not an installed one.
    pub custom: bool,
    pub glyphs: u64,
}

/// What the comparison found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The declared family drew the text (or a generic family was declared).
    Match,
    /// Another font drew all of it.
    Fallback { rendered: String },
    /// The declared family drew some of it, `fallback` the rest (`glyphs` glyphs).
    Glyphs { fallback: String, glyphs: u64 },
    /// Nothing was drawn, or Chromium did not tell.
    Unknown,
}

impl Verdict {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Match => "ok",
            Self::Fallback { .. } => "fallback",
            Self::Glyphs { .. } => "glyphs",
            Self::Unknown => "unknown",
        }
    }
}

/// `"Inter ASCII"` -> `Inter ASCII`.
fn unquote(text: &str) -> &str {
    let text = text.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = text
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    text
}

/// The families of a `font-family` stack, unquoted, in order.
#[must_use]
pub fn families(stack: &str) -> Vec<String> {
    let mut list = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in stack.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => {
                quote = None;
                current.push(c);
            }
            (None, '"' | '\'') => {
                quote = Some(c);
                current.push(c);
            }
            (None, ',') => {
                list.push(unquote(&current).to_owned());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        list.push(unquote(&current).to_owned());
    }
    list.retain(|family| !family.is_empty());
    list
}

/// Whether `family` is a generic family or a system keyword.
#[must_use]
pub fn generic(family: &str) -> bool {
    GENERIC.contains(&family.to_ascii_lowercase().as_str())
}

/// Compares what `stack` declares with what was drawn; `faces` are the page's web fonts (a web
/// font may carry another name inside its file than its `@font-face` alias).
#[must_use]
pub fn judge(stack: &str, faces: &[Face], fonts: &[RenderedFont]) -> Verdict {
    let declared = families(stack);
    let Some(first) = declared.first() else {
        return Verdict::Unknown;
    };
    if generic(first) {
        return Verdict::Match;
    }
    let drawn: Vec<&RenderedFont> = fonts.iter().filter(|font| font.glyphs > 0).collect();
    if drawn.is_empty() {
        return Verdict::Unknown;
    }
    let loaded_web = faces.iter().any(|face| {
        face.family.eq_ignore_ascii_case(first) && matches!(face.status.as_str(), "loaded")
    });
    let others: Vec<&String> = declared.iter().skip(1).collect();
    let is_declared = |font: &RenderedFont| {
        font.family.eq_ignore_ascii_case(first)
            || (loaded_web
                && font.custom
                && !others
                    .iter()
                    .any(|other| other.eq_ignore_ascii_case(&font.family)))
    };
    let (own, foreign): (Vec<&RenderedFont>, Vec<&RenderedFont>) =
        drawn.into_iter().partition(|font| is_declared(font));
    let Some(top) = foreign.iter().max_by_key(|font| font.glyphs) else {
        return Verdict::Match;
    };
    if own.is_empty() {
        return Verdict::Fallback {
            rendered: top.family.clone(),
        };
    }
    Verdict::Glyphs {
        fallback: top.family.clone(),
        glyphs: foreign.iter().map(|font| font.glyphs).sum(),
    }
}

fn send(
    host: &mut dyn PluginHost,
    view: ViewId,
    method: &str,
    params: &Value,
) -> Result<Value, Failure> {
    let answer = host.cdp_send(view, method, &params.to_string())?;
    Ok(serde_json::from_str(&answer).unwrap_or(Value::Null))
}

fn node(value: &Value) -> Option<u64> {
    value.as_u64().filter(|id| *id != 0)
}

/// The front-end node id of `view`'s document, without dropping anyone's node ids (above).
///
/// # Errors
///
/// The view has no CDP (Firefox, WebKit) or no accessibility tree.
pub fn document(host: &mut dyn PluginHost, view: ViewId) -> Result<u64, Failure> {
    let failed =
        || Failure::coded(FailureKind::Other, codes::HOST_FAILED).with_param("reason", "document");
    let tree = send(
        host,
        view,
        "Accessibility.getFullAXTree",
        &json!({ "depth": 1 }),
    )?;
    let backend = tree["nodes"][0]["backendDOMNodeId"]
        .as_u64()
        .ok_or_else(failed)?;
    let params = json!({ "backendNodeIds": [backend] });
    if let Ok(pushed) = send(host, view, "DOM.pushNodesByBackendIdsToFrontend", &params)
        && let Some(id) = node(&pushed["nodeIds"][0])
    {
        return Ok(id);
    }
    let document = send(host, view, "DOM.getDocument", &json!({ "depth": 0 }))?;
    node(&document["root"]["nodeId"]).ok_or_else(failed)
}

fn fonts_of(answer: &Value) -> Option<Vec<RenderedFont>> {
    let mut fonts: Vec<RenderedFont> = answer["fonts"]
        .as_array()?
        .iter()
        .map(|font| RenderedFont {
            family: cut(font["familyName"].as_str().unwrap_or_default(), MAX_NAME),
            custom: font["isCustomFont"].as_bool().unwrap_or(false),
            glyphs: font["glyphCount"].as_u64().unwrap_or(0),
        })
        .collect();
    fonts.sort_by_key(|font| std::cmp::Reverse(font.glyphs));
    fonts.truncate(MAX_FONTS);
    Some(fonts)
}

/// The fonts of the element `selector` names under `root`; `Err` when a step failed.
fn read_one(
    host: &mut dyn PluginHost,
    view: ViewId,
    root: u64,
    selector: &str,
) -> Result<Option<Vec<RenderedFont>>, Failure> {
    let found = send(
        host,
        view,
        "DOM.querySelector",
        &json!({ "nodeId": root, "selector": selector }),
    )?;
    let Some(id) = node(&found["nodeId"]) else {
        return Ok(None);
    };
    let answer = send(
        host,
        view,
        "CSS.getPlatformFontsForNode",
        &json!({ "nodeId": id }),
    )?;
    Ok(fonts_of(&answer))
}

/// The rendered fonts of each element `selectors` names in `view`: `None` where the element is
/// gone or Chromium did not tell.
///
/// # Errors
///
/// The view has no CDP (Firefox, WebKit) or its document could not be named.
pub fn read(
    host: &mut dyn PluginHost,
    view: ViewId,
    selectors: &[&str],
) -> Result<Vec<Option<Vec<RenderedFont>>>, Failure> {
    let mut root = document(host, view)?;
    let mut retried = false;
    let mut out = Vec::with_capacity(selectors.len());
    for selector in selectors {
        let fonts = match read_one(host, view, root, selector) {
            Ok(fonts) => fonts,
            Err(failure) if failure.code() == Some(codes::NOT_AVAILABLE) => return Err(failure),
            Err(_) if !retried => {
                retried = true;
                root = document(host, view)?;
                read_one(host, view, root, selector).ok().flatten()
            }
            Err(_) => None,
        };
        out.push(fonts);
    }
    Ok(out)
}

#[cfg(test)]
#[path = "rendered_tests.rs"]
mod tests;
