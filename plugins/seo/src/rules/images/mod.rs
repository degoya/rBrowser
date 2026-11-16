//! `images.*`, the alt text group `seo.images`, over the rendered DOM when there is one (lazy
//! images are in it as elements, loaded or not): every `<img>` - inside `<picture>` too, its alt
//! is the `<img>`'s - is missing its alt, has an empty one where the image is not decorative,
//! is a link without a name, or has a poor alt; the other image-like elements are
//! [`other`]. Answers the image list of the report and the panel.

use std::collections::BTreeMap;

use scraper::{ElementRef, Node};
use serde::Serialize;

use crate::audit::Page;
use crate::config::Config;
use crate::page::{sel, squash, text_of};
use crate::rules::{Element, Sink, short};

pub mod alt;
pub mod other;

/// The most images the list keeps.
pub const MAX_LISTED: usize = 500;

/// One `<img>` of the list: what it shows and which rules it broke (none: ok).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImageEntry {
    pub src: String,
    pub alt: Option<String>,
    pub selector: String,
    pub path: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// The rule ids found at the image; empty when it is fine.
    pub status: Vec<String>,
}

/// The image's source: `src`, a lazy loader's `data-src`, or the first `srcset` candidate.
fn source(img: ElementRef<'_>) -> String {
    let element = img.value();
    let srcset = |value: &str| {
        value
            .split(',')
            .next()
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned()
    };
    element
        .attr("src")
        .filter(|src| !src.trim().is_empty() && !src.starts_with("data:"))
        .map(|src| src.trim().to_owned())
        .or_else(|| element.attr("data-src").map(|src| src.trim().to_owned()))
        .or_else(|| element.attr("srcset").map(srcset))
        .or_else(|| element.attr("data-srcset").map(srcset))
        .unwrap_or_default()
        .chars()
        .take(2048)
        .collect()
}

/// The rendered size by path, else the `width` and `height` attributes.
pub fn size(
    element: ElementRef<'_>,
    path: &str,
    sizes: Option<&BTreeMap<String, (u32, u32)>>,
) -> Option<(u32, u32)> {
    sizes
        .and_then(|sizes| sizes.get(path).copied())
        .or_else(|| {
            let attr = |name: &str| {
                element
                    .value()
                    .attr(name)?
                    .trim()
                    .trim_end_matches("px")
                    .parse()
                    .ok()
            };
            Some((attr("width")?, attr("height")?))
        })
}

/// Whether the element is hidden from assistive technology or says it is decoration.
pub fn presentational(element: ElementRef<'_>) -> bool {
    let value = element.value();
    value
        .attr("aria-hidden")
        .is_some_and(|hidden| hidden.trim() == "true")
        || value.attr("role").is_some_and(|role| {
            role.split_ascii_whitespace()
                .next()
                .is_some_and(|role| role == "presentation" || role == "none")
        })
}

pub fn closest<'a>(
    element: ElementRef<'a>,
    test: impl Fn(ElementRef<'a>) -> bool,
) -> Option<ElementRef<'a>> {
    element
        .ancestors()
        .filter_map(ElementRef::wrap)
        .find(|ancestor| test(*ancestor))
}

fn link_of(img: ElementRef<'_>) -> Option<ElementRef<'_>> {
    closest(img, |element| {
        element.value().name() == "a" && element.value().attr("href").is_some()
    })
}

fn non_empty(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

/// Whether `link` has a name besides `img`: its text, its own label, or another image's alt.
fn link_has_name(link: ElementRef<'_>, img: ElementRef<'_>) -> bool {
    let value = link.value();
    !text_of(link).is_empty()
        || non_empty(value.attr("aria-label"))
        || non_empty(value.attr("aria-labelledby"))
        || non_empty(value.attr("title"))
        || link.select(&sel("img[alt], svg title")).any(|other| {
            other.id() != img.id()
                && (other.value().name() == "title" && !text_of(other).is_empty()
                    || non_empty(other.value().attr("alt")))
        })
}

/// The text of the node right beside an element in one direction (`next_siblings`,
/// `prev_siblings`): white space and comments skipped, an element's text; none when that node
/// has no text.
fn beside<'a>(mut nodes: impl Iterator<Item = ego_tree::NodeRef<'a, Node>>) -> Option<String> {
    let node = nodes.find(|node| match node.value() {
        Node::Text(text) => !text.trim().is_empty(),
        Node::Element(_) => true,
        _ => false,
    })?;
    let text = match node.value() {
        Node::Text(text) => squash(text),
        _ => ElementRef::wrap(node).map(text_of)?,
    };
    (!text.is_empty()).then_some(text)
}

/// The text right before and after `img` and its figure's caption, collapsed.
fn neighbours(img: ElementRef<'_>) -> Vec<String> {
    let mut texts: Vec<String> = [beside(img.next_siblings()), beside(img.prev_siblings())]
        .into_iter()
        .flatten()
        .collect();
    if let Some(figure) = closest(img, |element| element.value().name() == "figure") {
        texts.extend(figure.select(&sel("figcaption")).map(text_of));
    }
    texts
}

/// Runs the group over the primary document; answers the image list.
pub fn check(page: &Page<'_>, sink: &mut Sink<'_>) -> Vec<ImageEntry> {
    let Some((html, facts)) = page.primary() else {
        return Vec::new();
    };
    let sizes = page.rendered.as_ref().map(|(rendered, _)| &rendered.sizes);
    let mut list = Vec::new();
    let mut seen: BTreeMap<String, (String, String)> = BTreeMap::new();
    for img in html.select(&sel("img")) {
        let element = Element::of(img);
        let src = source(img);
        let alt = img.value().attr("alt");
        let found = check_image(
            img,
            &element,
            &src,
            facts.lang.as_deref(),
            sizes,
            page.config,
            &mut seen,
            sink,
        );
        if list.len() < MAX_LISTED {
            let (width, height) = size(img, &element.path, sizes).unzip();
            list.push(ImageEntry {
                src,
                alt: alt.map(|alt| short(alt, 300)),
                selector: element.selector,
                path: element.path,
                width,
                height,
                status: found,
            });
        }
    }
    other::check(html, page, sink);
    list
}

#[allow(
    clippy::too_many_arguments,
    reason = "one image's whole context, read once"
)]
fn check_image(
    img: ElementRef<'_>,
    element: &Element,
    src: &str,
    lang: Option<&str>,
    sizes: Option<&BTreeMap<String, (u32, u32)>>,
    config: &Config,
    seen: &mut BTreeMap<String, (String, String)>,
    sink: &mut Sink<'_>,
) -> Vec<String> {
    let mut found = Vec::new();
    let mut emit = |sink: &mut Sink<'_>, rule: &str, params: &[(&str, String)]| {
        if sink.keep(rule, rule, Some(element.clone()), params) {
            found.push(rule.to_owned());
        }
    };
    let src_param = ("src", short(src, 300));
    let unnamed_link = link_of(img).filter(|link| !link_has_name(*link, img));
    let Some(alt) = img.value().attr("alt") else {
        emit(sink, "images.alt_missing", std::slice::from_ref(&src_param));
        if unnamed_link.is_some() {
            emit(sink, "images.linked_no_name", &[src_param]);
        }
        return found;
    };
    if alt.trim().is_empty() {
        // Inside a link with a name of its own, the image is decoration of that link.
        if unnamed_link.is_some() {
            emit(sink, "images.linked_no_name", &[src_param]);
        } else if link_of(img).is_none() {
            let small = size(img, &element.path, sizes).is_some_and(|(width, height)| {
                width <= config.decorative_max_size && height <= config.decorative_max_size
            });
            if !small && !presentational(img) {
                emit(sink, "images.alt_empty", &[src_param]);
            }
        }
        return found;
    }
    let alt_param = ("alt", short(alt, 200));
    if alt::is_filename(alt, src) {
        emit(
            sink,
            "images.alt_filename",
            &[alt_param.clone(), src_param.clone()],
        );
    } else if alt::is_placeholder(alt, &config.placeholders) {
        emit(
            sink,
            "images.alt_placeholder",
            std::slice::from_ref(&alt_param),
        );
    } else if let Some(prefix) = alt::redundant_prefix(alt) {
        emit(
            sink,
            "images.alt_prefix",
            &[alt_param.clone(), ("prefix", prefix.to_owned())],
        );
    }
    let length = alt.chars().count();
    if length > config.max_alt_length {
        emit(
            sink,
            "images.alt_too_long",
            &[
                alt_param.clone(),
                ("length", length.to_string()),
                ("max", config.max_alt_length.to_string()),
            ],
        );
    }
    let key = alt::normalize(alt);
    match seen.get(&key) {
        Some((other_src, other_path)) if other_src != src => emit(
            sink,
            "images.alt_duplicate",
            &[alt_param.clone(), ("other", other_path.clone())],
        ),
        Some(_) => {}
        None => {
            seen.insert(key.clone(), (src.to_owned(), element.path.clone()));
        }
    }
    if neighbours(img)
        .iter()
        .any(|text| alt::normalize(text) == key)
    {
        emit(
            sink,
            "images.alt_redundant",
            std::slice::from_ref(&alt_param),
        );
    }
    if let Some(lang) = lang
        && let Some(language) = alt::foreign_language(alt, lang)
    {
        emit(
            sink,
            "images.alt_language",
            &[
                alt_param,
                ("language", language.to_owned()),
                ("lang", lang.to_owned()),
            ],
        );
    }
    found
}

/// The statistic "X of Y images ok".
#[must_use]
pub fn ok_count(list: &[ImageEntry]) -> usize {
    list.iter().filter(|entry| entry.status.is_empty()).count()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
