//! The image-like elements besides `<img>`: `<input type="image">` and `<area href>` need an
//! alt text, an inline `<svg>` with content a `<title>` or label, and a large CSS background
//! image without text over it probably carries content without any alternative (a hint, from
//! the rendered page only).

use scraper::{ElementRef, Html, Node};

use crate::audit::Page;
use crate::page::{sel, text_of};
use crate::rules::images::{closest, presentational};
use crate::rules::{Element, Sink, short};

fn named(element: ElementRef<'_>) -> bool {
    ["alt", "aria-label", "aria-labelledby", "title"]
        .iter()
        .any(|name| {
            element
                .value()
                .attr(name)
                .is_some_and(|value| !value.trim().is_empty())
        })
}

/// Whether an `<svg>` draws something: an element other than its metadata.
fn draws(svg: ElementRef<'_>) -> bool {
    svg.descendent_elements().skip(1).any(|child| {
        !matches!(
            child.value().name(),
            "title" | "desc" | "defs" | "metadata" | "style"
        )
    })
}

/// Whether the `<svg>` has a name: a `<title>` child with text, or a label.
fn svg_named(svg: ElementRef<'_>) -> bool {
    named(svg)
        || svg
            .child_elements()
            .any(|child| child.value().name() == "title" && !text_of(child).is_empty())
}

/// Whether the node right beside the `<svg>` in one direction is inline text: a text node or a
/// `span` with text (a heading or a block beside it is no label).
fn inline_text<'a>(mut nodes: impl Iterator<Item = ego_tree::NodeRef<'a, Node>>) -> bool {
    nodes
        .find(|node| match node.value() {
            Node::Text(text) => !text.trim().is_empty(),
            Node::Element(_) => true,
            _ => false,
        })
        .is_some_and(|node| match node.value() {
            Node::Text(_) => true,
            Node::Element(element) => {
                element.name() == "span"
                    && ElementRef::wrap(node).is_some_and(|span| !text_of(span).is_empty())
            }
            _ => false,
        })
}

/// Whether the `<svg>` is an icon beside its label: inline text right next to it, or a link,
/// button, label or summary around it with a text or name of its own.
fn beside_text(svg: ElementRef<'_>) -> bool {
    inline_text(svg.next_siblings())
        || inline_text(svg.prev_siblings())
        || closest(svg, |element| {
            matches!(element.value().name(), "a" | "button" | "label" | "summary")
        })
        .is_some_and(|control| !text_of(control).is_empty() || named(control))
}

pub fn check(html: &Html, page: &Page<'_>, sink: &mut Sink<'_>) {
    for input in html.select(&sel("input[type]")) {
        let image = input
            .value()
            .attr("type")
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("image"));
        if image && !named(input) {
            sink.emit(
                "images.input_alt_missing",
                "images.input_alt_missing",
                Some(Element::of(input)),
                &[(
                    "src",
                    short(input.value().attr("src").unwrap_or_default(), 300),
                )],
            );
        }
    }
    for area in html.select(&sel("area[href]")) {
        if !named(area) {
            sink.emit(
                "images.area_alt_missing",
                "images.area_alt_missing",
                Some(Element::of(area)),
                &[(
                    "href",
                    short(area.value().attr("href").unwrap_or_default(), 300),
                )],
            );
        }
    }
    for svg in html.select(&sel("svg")) {
        let nested = svg
            .ancestors()
            .filter_map(ElementRef::wrap)
            .any(|ancestor| ancestor.value().name() == "svg");
        if nested || presentational(svg) || !draws(svg) || svg_named(svg) || beside_text(svg) {
            continue;
        }
        sink.emit(
            "images.svg_name_missing",
            "images.svg_name_missing",
            Some(Element::of(svg)),
            &[],
        );
    }
    if let Some((rendered, _)) = &page.rendered {
        for background in &rendered.backgrounds {
            sink.emit(
                "images.background",
                "images.background",
                Some(Element::at(&background.path)),
                &[
                    ("url", short(&background.url, 300)),
                    ("width", background.width.to_string()),
                    ("height", background.height.to_string()),
                ],
            );
        }
    }
}
