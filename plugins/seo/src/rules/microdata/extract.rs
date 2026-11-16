//! The items of a document's Microdata (`itemscope`, `itemtype`, `itemprop`) and RDFa Lite
//! (`vocab`, `typeof`, `property`), read with `scraper`: each top-level item, its types (a
//! schema.org type by its short name) and its properties - text values and nested items. Not
//! read: `itemref`, `itemid`, RDFa `prefix` other than `schema:`, `resource`.

use std::collections::BTreeMap;

use scraper::{ElementRef, Html};

use crate::page::{sel, squash};
use crate::path::element_path;

/// The most items of one document read, nested ones included.
pub const MAX_ITEMS: usize = 500;
/// The deepest nesting followed.
const MAX_DEPTH: usize = 16;

/// Which syntax an item is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syntax {
    Microdata,
    Rdfa,
}

impl Syntax {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Microdata => "Microdata",
            Self::Rdfa => "RDFa",
        }
    }

    fn scope(self) -> &'static str {
        match self {
            Self::Microdata => "itemscope",
            Self::Rdfa => "typeof",
        }
    }

    fn prop(self) -> &'static str {
        match self {
            Self::Microdata => "itemprop",
            Self::Rdfa => "property",
        }
    }
}

/// A property's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Item(Box<Item>),
}

/// One item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub syntax: Syntax,
    /// The schema.org types by short name (`Product`).
    pub types: Vec<String>,
    /// Types of another vocabulary, as written.
    pub other_types: Vec<String>,
    pub props: BTreeMap<String, Vec<Value>>,
    pub path: String,
}

impl Item {
    /// The first text value of `name`.
    #[must_use]
    pub fn text(&self, name: &str) -> Option<&str> {
        self.props.get(name)?.iter().find_map(|value| match value {
            Value::Text(text) => Some(text.as_str()),
            Value::Item(_) => None,
        })
    }
}

/// A schema.org type IRI or name, by its short name; `None` for another vocabulary.
fn schema_type(token: &str, vocab: Option<&str>) -> Option<String> {
    let token = token.trim();
    for prefix in [
        "https://schema.org/",
        "http://schema.org/",
        "https://www.schema.org/",
        "http://www.schema.org/",
        "schema:",
    ] {
        if let Some(name) = token.strip_prefix(prefix) {
            return Some(name.trim_end_matches('/').to_owned());
        }
    }
    let plain = !token.contains(':') && !token.contains('/');
    (plain && vocab.is_some_and(|vocab| vocab.contains("schema.org"))).then(|| token.to_owned())
}

fn property_name(token: &str) -> String {
    let token = token.trim();
    schema_type(token, None).unwrap_or_else(|| token.to_owned())
}

/// The `vocab` in force at `element` (its own or its nearest ancestor's).
fn vocab_of(element: ElementRef<'_>) -> Option<String> {
    std::iter::once(element)
        .chain(element.ancestors().filter_map(ElementRef::wrap))
        .find_map(|e| e.value().attr("vocab").map(str::to_owned))
}

/// A property element's value as the syntax reads it.
fn text_value(element: ElementRef<'_>, syntax: Syntax) -> String {
    let e = element.value();
    if syntax == Syntax::Rdfa
        && let Some(content) = e.attr("content")
    {
        return squash(content);
    }
    let attr = match e.name() {
        "meta" => "content",
        "audio" | "embed" | "iframe" | "img" | "source" | "track" | "video" => "src",
        "a" | "area" | "link" => "href",
        "object" => "data",
        "data" | "meter" => "value",
        "time" if e.attr("datetime").is_some() => "datetime",
        _ => return squash(&element.text().collect::<String>()),
    };
    squash(e.attr(attr).unwrap_or_default())
}

fn item(element: ElementRef<'_>, syntax: Syntax, depth: usize, count: &mut usize) -> Item {
    *count += 1;
    let attr = match syntax {
        Syntax::Microdata => "itemtype",
        Syntax::Rdfa => "typeof",
    };
    let vocab = match syntax {
        Syntax::Microdata => None,
        Syntax::Rdfa => vocab_of(element),
    };
    let mut item = Item {
        syntax,
        types: Vec::new(),
        other_types: Vec::new(),
        props: BTreeMap::new(),
        path: element_path(element),
    };
    for token in element
        .value()
        .attr(attr)
        .unwrap_or_default()
        .split_ascii_whitespace()
    {
        match schema_type(token, vocab.as_deref()) {
            Some(name) => item.types.push(name),
            None => item.other_types.push(token.to_owned()),
        }
    }
    props(element, syntax, depth, count, &mut item.props);
    item
}

fn props(
    element: ElementRef<'_>,
    syntax: Syntax,
    depth: usize,
    count: &mut usize,
    out: &mut BTreeMap<String, Vec<Value>>,
) {
    for child in element.children().filter_map(ElementRef::wrap) {
        let scoped = child.value().attr(syntax.scope()).is_some();
        if let Some(names) = child.value().attr(syntax.prop()) {
            let value = if scoped && depth < MAX_DEPTH && *count < MAX_ITEMS {
                Value::Item(Box::new(item(child, syntax, depth + 1, count)))
            } else {
                Value::Text(text_value(child, syntax))
            };
            for name in names.split_ascii_whitespace() {
                out.entry(property_name(name))
                    .or_default()
                    .push(value.clone());
            }
        }
        if !scoped {
            props(child, syntax, depth, count, out);
        }
    }
}

/// The top-level items of `html`: Microdata first, then RDFa, each in document order.
#[must_use]
pub fn items(html: &Html) -> Vec<Item> {
    let mut count = 0;
    let mut out = Vec::new();
    for (syntax, selector) in [
        (Syntax::Microdata, "[itemscope]:not([itemprop])"),
        (Syntax::Rdfa, "[typeof]:not([property])"),
    ] {
        for element in html.select(&sel(selector)) {
            if count >= MAX_ITEMS {
                return out;
            }
            out.push(item(element, syntax, 0, &mut count));
        }
    }
    out
}

#[cfg(test)]
#[path = "extract_tests.rs"]
mod tests;
