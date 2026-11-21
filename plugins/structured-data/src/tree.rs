//! The structured data of a page as one tree: every top-level JSON-LD object (each entry of an
//! `@graph` on its own) and every top-level Microdata and RDFa item is a [`Node`] with its
//! schema.org types, its `@id`, where it comes from and its properties; a value is text, a
//! number, a boolean, a nested node or a reference (`{"@id": ...}`) to a node elsewhere on the
//! page, resolved by [`finish`]. Nodes are numbered `n0`, `n1`, ... in document order, depth
//! first, so the panel and the findings name the same node.

use std::collections::BTreeMap;

use schema_org::microdata::{self, Item};
use serde::Serialize;
use serde_json::{Map, Value};

/// The most nodes read from one page, nested ones included.
pub const MAX_NODES: usize = 500;
/// The deepest nesting followed.
pub const MAX_DEPTH: usize = 16;
/// The longest text kept of one value, in characters.
pub const MAX_TEXT: usize = 1024;

/// The syntax a node is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Syntax {
    #[serde(rename = "json-ld")]
    JsonLd,
    #[serde(rename = "microdata")]
    Microdata,
    #[serde(rename = "rdfa")]
    Rdfa,
}

/// Where a node is written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Source {
    pub syntax: Syntax,
    /// The JSON-LD block's index on the page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<usize>,
    /// The element's path: the `<script>` of a JSON-LD node, the item's own element otherwise.
    pub path: String,
}

/// One value of a property.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Val {
    Text {
        text: String,
    },
    Number {
        text: String,
    },
    Bool {
        value: bool,
    },
    Node {
        node: Box<Node>,
    },
    /// `{"@id": ...}`: the node of the page with that `@id`, when there is one.
    Ref {
        #[serde(rename = "atId")]
        at_id: String,
        target: Option<String>,
    },
}

/// One property and its values, in the page's order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Prop {
    /// The schema.org name (`headline`), or the key as written for another vocabulary.
    pub name: String,
    pub values: Vec<Val>,
}

/// One node of the tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// `n<index>`, set by [`finish`].
    pub id: String,
    /// The schema.org types by short name.
    pub types: Vec<String>,
    /// Types of another vocabulary, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub other_types: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_id: Option<String>,
    pub source: Source,
    /// The JSON pointer within the block (`/@graph/1/author`), or the property path within the
    /// top-level item (`/offers/0`); empty for a top-level node of its own.
    pub pointer: String,
    pub props: Vec<Prop>,
    /// Whether its properties are schema.org's (a schema.org type, or a JSON-LD block with the
    /// schema.org context): only then are they held to the vocabulary.
    #[serde(skip)]
    pub schema: bool,
}

impl Node {
    /// The values of `name`; none when the node lacks it.
    #[must_use]
    pub fn values(&self, name: &str) -> &[Val] {
        self.props
            .iter()
            .find(|prop| prop.name == name)
            .map(|prop| prop.values.as_slice())
            .unwrap_or_default()
    }
}

/// A JSON pointer step for `key` (`~` and `/` escaped).
#[must_use]
pub fn pointer_step(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn cut(text: &str) -> String {
    text.chars().take(MAX_TEXT).collect()
}

/// A type or property name as a schema.org short name: an IRI or `schema:` name of schema.org,
/// or a plain name in a block with the schema.org context.
fn schema_name(token: &str, context: bool) -> Option<String> {
    let token = token.trim();
    if !token.contains(':') && !token.contains('/') {
        return context.then(|| token.to_owned());
    }
    schema_org::short_name(token).map(str::to_owned)
}

/// Whether the `@context` of `value` (or of an object of its top-level array) is schema.org.
fn context_is_schema(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.iter().any(context_is_schema),
        Value::Object(map) => map
            .get("@context")
            .is_some_and(|context| context.to_string().contains("schema.org")),
        _ => false,
    }
}

/// What a block's nodes share.
struct Ctx {
    source: Source,
    schema: bool,
}

/// Builds the nodes of one page, counting them against [`MAX_NODES`].
#[derive(Debug, Default)]
pub struct Builder {
    count: usize,
    /// A node was left out: too many or too deep.
    pub truncated: bool,
}

impl Builder {
    /// Whether no further node fits.
    #[must_use]
    pub fn full(&self) -> bool {
        self.count >= MAX_NODES
    }

    /// The top-level nodes of the JSON-LD block `block` (its `<script>` at `path`).
    pub fn json_ld(&mut self, value: &Value, block: usize, path: &str) -> Vec<Node> {
        let ctx = Ctx {
            source: Source {
                syntax: Syntax::JsonLd,
                block: Some(block),
                path: path.to_owned(),
            },
            schema: context_is_schema(value),
        };
        let mut out = Vec::new();
        self.top(value, String::new(), &ctx, &mut out);
        out
    }

    fn top(&mut self, value: &Value, pointer: String, ctx: &Ctx, out: &mut Vec<Node>) {
        match value {
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    self.top(item, format!("{pointer}/{index}"), ctx, out);
                }
            }
            Value::Object(map) => {
                if let Some(graph) = map.get("@graph") {
                    self.top(graph, format!("{pointer}/@graph"), ctx, out);
                }
                let own = map.contains_key("@type") || map.keys().any(|key| !key.starts_with('@'));
                if own && let Some(node) = self.node(map, pointer, ctx, 0) {
                    out.push(node);
                }
            }
            _ => {}
        }
    }

    fn node(
        &mut self,
        map: &Map<String, Value>,
        pointer: String,
        ctx: &Ctx,
        depth: usize,
    ) -> Option<Node> {
        if depth > MAX_DEPTH || self.full() {
            self.truncated = true;
            return None;
        }
        self.count += 1;
        let tokens: Vec<&str> = match map.get("@type") {
            Some(Value::String(name)) => vec![name.as_str()],
            Some(Value::Array(names)) => names.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        let mut types = Vec::new();
        let mut other_types = Vec::new();
        for token in tokens {
            match schema_name(token, ctx.schema) {
                Some(name) => types.push(name),
                None => other_types.push(token.trim().to_owned()),
            }
        }
        let schema = !types.is_empty() || (other_types.is_empty() && ctx.schema);
        let mut props = Vec::new();
        for (key, value) in map.iter().filter(|(key, _)| !key.starts_with('@')) {
            let mut values = Vec::new();
            let at = format!("{pointer}/{}", pointer_step(key));
            self.values(value, at, ctx, depth, &mut values);
            if !values.is_empty() {
                props.push(Prop {
                    name: schema_name(key, ctx.schema).unwrap_or_else(|| key.clone()),
                    values,
                });
            }
        }
        Some(Node {
            id: String::new(),
            types,
            other_types,
            at_id: map.get("@id").and_then(Value::as_str).map(cut),
            source: ctx.source.clone(),
            pointer,
            props,
            schema,
        })
    }

    fn values(
        &mut self,
        value: &Value,
        pointer: String,
        ctx: &Ctx,
        depth: usize,
        out: &mut Vec<Val>,
    ) {
        match value {
            Value::Null => {}
            Value::Bool(value) => out.push(Val::Bool { value: *value }),
            Value::Number(number) => out.push(Val::Number {
                text: number.to_string(),
            }),
            Value::String(text) => out.push(Val::Text { text: cut(text) }),
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    self.values(item, format!("{pointer}/{index}"), ctx, depth, out);
                }
            }
            Value::Object(map) => {
                if let Some(inner) = map.get("@value") {
                    self.values(inner, pointer, ctx, depth, out);
                } else if let Some((key, list)) = ["@list", "@set"]
                    .into_iter()
                    .find_map(|key| map.get(key).map(|list| (key, list)))
                {
                    self.values(list, format!("{pointer}/{key}"), ctx, depth, out);
                } else if let (1, Some(Value::String(at_id))) = (map.len(), map.get("@id")) {
                    out.push(Val::Ref {
                        at_id: cut(at_id),
                        target: None,
                    });
                } else if let Some(node) = self.node(map, pointer, ctx, depth + 1) {
                    out.push(Val::Node {
                        node: Box::new(node),
                    });
                }
            }
        }
    }

    /// The node of a Microdata or RDFa item at `pointer` within its top-level item. The caller
    /// asks [`Builder::full`] before a top-level item; a nested item past [`MAX_NODES`] is left
    /// out and [`Builder::truncated`] set.
    pub fn item(&mut self, item: &Item, pointer: String) -> Node {
        self.count += 1;
        let syntax = match item.syntax {
            microdata::Syntax::Microdata => Syntax::Microdata,
            microdata::Syntax::Rdfa => Syntax::Rdfa,
        };
        let mut props = Vec::new();
        for (name, values) in &item.props {
            let mut kept = Vec::new();
            for (index, value) in values.iter().enumerate() {
                match value {
                    microdata::Value::Text(text) => kept.push(Val::Text { text: cut(text) }),
                    microdata::Value::Item(_) if self.full() => self.truncated = true,
                    microdata::Value::Item(nested) => {
                        let at = format!("{pointer}/{}/{index}", pointer_step(name));
                        kept.push(Val::Node {
                            node: Box::new(self.item(nested, at)),
                        });
                    }
                }
            }
            if !kept.is_empty() {
                props.push(Prop {
                    name: name.clone(),
                    values: kept,
                });
            }
        }
        Node {
            id: String::new(),
            types: item.types.clone(),
            other_types: item.other_types.clone(),
            at_id: None,
            source: Source {
                syntax,
                block: None,
                path: item.path.clone(),
            },
            pointer,
            props,
            schema: !item.types.is_empty(),
        }
    }
}

/// Calls `visit` with every node of `nodes`, nested ones included, depth first.
pub fn each<'a>(nodes: &'a [Node], visit: &mut impl FnMut(&'a Node)) {
    for node in nodes {
        visit(node);
        for prop in &node.props {
            for value in &prop.values {
                if let Val::Node { node } = value {
                    each(std::slice::from_ref(node.as_ref()), visit);
                }
            }
        }
    }
}

fn number(node: &mut Node, next: &mut usize) {
    node.id = format!("n{next}");
    *next += 1;
    for prop in &mut node.props {
        for value in &mut prop.values {
            if let Val::Node { node } = value {
                number(node, next);
            }
        }
    }
}

fn resolve(node: &mut Node, ids: &BTreeMap<String, String>) {
    for prop in &mut node.props {
        for value in &mut prop.values {
            match value {
                Val::Ref { at_id, target } => *target = ids.get(at_id.as_str()).cloned(),
                Val::Node { node } => resolve(node, ids),
                _ => {}
            }
        }
    }
}

/// Numbers the nodes and resolves every reference to the first node defining its `@id` (one
/// with a type or a property).
pub fn finish(nodes: &mut [Node]) {
    let mut next = 0;
    for node in nodes.iter_mut() {
        number(node, &mut next);
    }
    let mut ids = BTreeMap::new();
    each(nodes, &mut |node: &Node| {
        if let Some(at_id) = &node.at_id
            && (!node.types.is_empty() || !node.props.is_empty())
        {
            ids.entry(at_id.clone()).or_insert_with(|| node.id.clone());
        }
    });
    for node in nodes.iter_mut() {
        resolve(node, &ids);
    }
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
