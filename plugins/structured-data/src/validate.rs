//! The page's structured data held to the schema.org vocabulary and the rich-result table: per
//! block whether it is JSON, per node its types, the context of a JSON-LD node, every schema.org
//! property (known, for this type, a value of the expected kind, not superseded) and - when
//! `structured-data.richResults` is on - the properties the rich result of its type needs and
//! recommends. Each issue names its rule, node, property and place; [`crate::rules`] makes
//! findings of them.

use std::collections::BTreeMap;

use schema_org::rich::Table;
use schema_org::short_name;
use schema_org::vocab::Vocabulary;

use crate::formats;
use crate::page::Page;
use crate::rules::{
    JSONLD_CONTEXT, JSONLD_INVALID, PROPERTY_DOMAIN, PROPERTY_RANGE, PROPERTY_SUPERSEDED,
    PROPERTY_UNKNOWN, RICH_RECOMMENDED, RICH_REQUIRED, RICH_REQUIRED_ANY, TYPE_MISSING,
    TYPE_UNKNOWN,
};
use crate::tree::{self, Node, Syntax, Val, pointer_step};

/// The longest value named in an issue, in characters.
const MAX_VALUE: usize = 100;

/// What the validation covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// The rich-result needs and recommendations (`structured-data.richResults`).
    pub rich: bool,
}

/// One issue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub rule: &'static str,
    /// The node's id (`n3`); `None` for a block that is no JSON.
    pub node: Option<String>,
    pub property: Option<String>,
    /// Where it is, stable while the page does not change: `jsonld/<block>#<pointer>[/<property>]`,
    /// `<syntax>:<element path>#<pointer>[/<property>]`.
    pub place: String,
    /// The element's path: the `<script>` or the item's element.
    pub element: String,
    pub params: Vec<(&'static str, String)>,
}

/// Every issue of `page`: the blocks first, then each node, depth first.
#[must_use]
pub fn validate(page: &Page, vocab: &Vocabulary, table: &Table, options: Options) -> Vec<Issue> {
    let mut out = Vec::new();
    for block in &page.blocks {
        if let Some(reason) = &block.error {
            out.push(Issue {
                rule: JSONLD_INVALID,
                node: None,
                property: None,
                place: format!("jsonld/{}", block.index),
                element: block.path.clone(),
                params: vec![
                    ("block", (block.index + 1).to_string()),
                    ("reason", reason.clone()),
                ],
            });
        }
    }
    let mut by_id = BTreeMap::new();
    tree::each(&page.nodes, &mut |node| {
        by_id.insert(node.id.clone(), node);
    });
    let checker = Checker {
        vocab,
        table,
        options,
        by_id,
    };
    for node in &page.nodes {
        checker.node(node, true, &mut out);
    }
    out
}

fn syntax_name(syntax: Syntax) -> &'static str {
    match syntax {
        Syntax::JsonLd => "JSON-LD",
        Syntax::Microdata => "Microdata",
        Syntax::Rdfa => "RDFa",
    }
}

/// The node's place (see [`Issue::place`]).
#[must_use]
pub fn place_of(node: &Node) -> String {
    match (node.source.syntax, node.source.block) {
        (Syntax::JsonLd, Some(block)) => format!("jsonld/{block}#{}", node.pointer),
        (Syntax::Rdfa, _) => format!("rdfa:{}#{}", node.source.path, node.pointer),
        _ => format!("microdata:{}#{}", node.source.path, node.pointer),
    }
}

fn plain(name: &str) -> bool {
    !name.contains(':') && !name.contains('/')
}

fn describe(value: &Val) -> String {
    let text = match value {
        Val::Text { text } | Val::Number { text } => text.clone(),
        Val::Bool { value } => value.to_string(),
        Val::Node { node } => node.types.join(", "),
        Val::Ref { at_id, .. } => at_id.clone(),
    };
    text.chars().take(MAX_VALUE).collect()
}

struct Checker<'a> {
    vocab: &'a Vocabulary,
    table: &'a Table,
    options: Options,
    by_id: BTreeMap<String, &'a Node>,
}

impl Checker<'_> {
    fn node(&self, node: &Node, top: bool, out: &mut Vec<Issue>) {
        let base = place_of(node);
        let issue = |rule: &'static str,
                     property: Option<&str>,
                     params: Vec<(&'static str, String)>| Issue {
            rule,
            node: Some(node.id.clone()),
            property: property.map(str::to_owned),
            place: match property {
                Some(name) => format!("{base}/{}", pointer_step(name)),
                None => base.clone(),
            },
            element: node.source.path.clone(),
            params,
        };
        if top && node.types.is_empty() && node.other_types.is_empty() {
            let syntax = syntax_name(node.source.syntax).to_owned();
            out.push(issue(TYPE_MISSING, None, vec![("syntax", syntax)]));
        }
        if node.source.syntax == Syntax::JsonLd
            && !node.schema
            && node.other_types.iter().any(|name| plain(name))
        {
            let types = node.other_types.join(", ");
            out.push(issue(JSONLD_CONTEXT, None, vec![("type", types)]));
        }
        for name in node.types.iter().filter(|name| !self.vocab.is_type(name)) {
            out.push(issue(TYPE_UNKNOWN, None, vec![("type", name.clone())]));
        }
        let types = node.types.join(", ");
        let known =
            !node.types.is_empty() && node.types.iter().all(|name| self.vocab.is_type(name));
        for prop in node
            .props
            .iter()
            .filter(|prop| node.schema && plain(&prop.name))
        {
            let name = prop.name.as_str();
            let Some(definition) = self.vocab.property(name) else {
                let params = vec![("type", types.clone()), ("property", name.to_owned())];
                out.push(issue(PROPERTY_UNKNOWN, Some(name), params));
                continue;
            };
            if known && !self.vocab.allows(&node.types, definition) {
                let params = vec![("type", types.clone()), ("property", name.to_owned())];
                out.push(issue(PROPERTY_DOMAIN, Some(name), params));
            }
            if let Some(next) = &definition.superseded_by {
                let params = vec![("property", name.to_owned()), ("replacement", next.clone())];
                out.push(issue(PROPERTY_SUPERSEDED, Some(name), params));
            }
            if let Some(value) = prop
                .values
                .iter()
                .find(|value| !self.fits(&definition.ranges, value))
            {
                let params = vec![
                    ("property", name.to_owned()),
                    ("expected", definition.ranges.join(", ")),
                    ("value", describe(value)),
                ];
                out.push(issue(PROPERTY_RANGE, Some(name), params));
            }
        }
        if self.options.rich {
            self.rich(node, &issue, out);
        }
        for prop in &node.props {
            for value in &prop.values {
                if let Val::Node { node } = value {
                    self.node(node, false, out);
                }
            }
        }
    }

    fn rich(
        &self,
        node: &Node,
        issue: &dyn Fn(&'static str, Option<&str>, Vec<(&'static str, String)>) -> Issue,
        out: &mut Vec<Issue>,
    ) {
        let has = |name: &str| !node.values(name).is_empty();
        for name in &node.types {
            if let Some(required) = self.table.required.get(name) {
                for property in required.all.iter().filter(|property| !has(property)) {
                    let params = vec![("type", name.clone()), ("property", property.clone())];
                    out.push(issue(RICH_REQUIRED, Some(property.as_str()), params));
                }
                for group in &required.any {
                    if !group.iter().any(|property| has(property)) {
                        let params = vec![("type", name.clone()), ("properties", group.join(", "))];
                        let mut found = issue(RICH_REQUIRED_ANY, None, params);
                        found.place = format!("{}/{}", found.place, group.join("|"));
                        out.push(found);
                    }
                }
            }
            for property in self
                .table
                .recommended
                .get(name)
                .into_iter()
                .flatten()
                .filter(|property| !has(property))
            {
                let params = vec![("type", name.clone()), ("property", property.clone())];
                out.push(issue(RICH_RECOMMENDED, Some(property.as_str()), params));
            }
        }
    }

    /// Whether `value` is of one of the kinds `ranges` names.
    fn fits(&self, ranges: &[String], value: &Val) -> bool {
        if ranges.is_empty() {
            return true;
        }
        match value {
            Val::Text { text } | Val::Number { text } => self.text_fits(ranges, text),
            Val::Bool { value } => self.text_fits(ranges, if *value { "true" } else { "false" }),
            Val::Node { node } => self.node_fits(ranges, &node.types),
            // A reference to a node elsewhere counts as that node; one outside the page as a URL.
            Val::Ref { target, .. } => target
                .as_deref()
                .and_then(|id| self.by_id.get(id))
                .is_none_or(|node| self.node_fits(ranges, &node.types)),
        }
    }

    /// A node fits a range it is or is below; one without a type, or of an unknown type (named
    /// on its own), is not held to it.
    fn node_fits(&self, ranges: &[String], types: &[String]) -> bool {
        types.is_empty()
            || types.iter().any(|name| {
                !self.vocab.is_type(name) || ranges.iter().any(|range| self.vocab.is_a(name, range))
            })
    }

    /// Text fits a data type it is written as, a member of an expected enumeration, and - as
    /// schema.org allows - any other class (a name in place of an item).
    fn text_fits(&self, ranges: &[String], text: &str) -> bool {
        let text = text.trim();
        ranges
            .iter()
            .any(|range| match self.vocab.data_type_of(range).as_deref() {
                Some("Number") if range == "Integer" => text.parse::<i64>().is_ok(),
                Some("Number") => text.parse::<f64>().is_ok_and(f64::is_finite),
                Some("Boolean") => formats::is_boolean(text),
                Some("Date") => formats::is_date(text) || formats::is_date_time(text),
                Some("DateTime") => formats::is_date_time(text),
                Some("Time") => formats::is_time(text),
                Some(_) => true,
                None if self.vocab.is_enumeration(range) => short_name(text).is_some_and(|name| {
                    self.vocab
                        .enumerations_of(name)
                        .iter()
                        .any(|enumeration| self.vocab.is_a(enumeration, range))
                }),
                None => true,
            })
    }
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
