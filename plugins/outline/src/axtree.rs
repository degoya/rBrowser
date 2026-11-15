//! The accessibility tree of a Chromium view (CDP `Accessibility.getFullAXTree`), as a flat list
//! in tree order with each node's depth - the panel draws it as an ARIA tree. Ignored nodes and
//! the text leaves (`StaticText`, `InlineTextBox`, `LineBreak`) are left out, nameless `generic`
//! and `none` nodes too; their children move up. Firefox and WebKit views have no CDP: their
//! tree is `not_available`.

use std::collections::HashMap;

use rb_plugin_api::{Failure, PluginHost, ViewId};
use serde::Serialize;
use serde_json::{Value, json};

use crate::page::{MAX_TEXT, cut};

/// The most nodes listed per view.
pub const MAX_NODES: usize = 2000;
/// The deepest node listed.
pub const MAX_DEPTH: usize = 64;

const LEAVES: [&str; 3] = ["StaticText", "InlineTextBox", "LineBreak"];

/// One node of the tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub depth: usize,
    pub role: String,
    pub name: String,
    /// The DOM node behind it, for the mark (`DOM.*` take `backendNodeId`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_node_id: Option<u64>,
}

/// A view's tree.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub truncated: bool,
}

fn value_of(node: &Value, field: &str) -> String {
    cut(node[field]["value"].as_str().unwrap_or_default(), MAX_TEXT)
}

fn kept(node: &Value) -> bool {
    if node["ignored"].as_bool().unwrap_or(false) {
        return false;
    }
    let role = node["role"]["value"].as_str().unwrap_or_default();
    if LEAVES.contains(&role) {
        return false;
    }
    !(matches!(role, "generic" | "none" | "") && value_of(node, "name").is_empty())
}

/// The tree of a `getFullAXTree` answer.
#[must_use]
pub fn tree(answer: &Value) -> Tree {
    let empty = Vec::new();
    let nodes = answer["nodes"].as_array().unwrap_or(&empty);
    let by_id: HashMap<&str, &Value> = nodes
        .iter()
        .filter_map(|node| Some((node["nodeId"].as_str()?, node)))
        .collect();
    let roots: Vec<&Value> = nodes
        .iter()
        .filter(|node| {
            node["parentId"]
                .as_str()
                .is_none_or(|parent| !by_id.contains_key(parent))
        })
        .collect();
    let mut tree = Tree::default();
    // Depth first, the children in their order; a stack of (node, depth), the last child first.
    let mut stack: Vec<(&Value, usize)> = roots.into_iter().rev().map(|node| (node, 0)).collect();
    let mut visited = 0usize;
    while let Some((node, depth)) = stack.pop() {
        visited += 1;
        if visited > nodes.len() {
            break; // A cycle in the page's answer.
        }
        let keep = kept(node);
        if keep {
            if tree.nodes.len() == MAX_NODES {
                tree.truncated = true;
                break;
            }
            tree.nodes.push(Node {
                depth,
                role: value_of(node, "role"),
                name: value_of(node, "name"),
                backend_node_id: node["backendDOMNodeId"].as_u64(),
            });
        }
        let child_depth = if keep { depth + 1 } else { depth };
        if child_depth > MAX_DEPTH {
            tree.truncated = true;
            continue;
        }
        if let Some(children) = node["childIds"].as_array() {
            for child in children.iter().rev() {
                if let Some(child) = child.as_str().and_then(|id| by_id.get(id)) {
                    stack.push((child, child_depth));
                }
            }
        }
    }
    tree
}

/// The tree of `view`.
///
/// # Errors
///
/// The host's failure (no CDP for the view, the domain refused) or a malformed answer.
pub fn read(host: &mut dyn PluginHost, view: ViewId) -> Result<Tree, Failure> {
    let answer = host.cdp_send(view, "Accessibility.getFullAXTree", &json!({}).to_string())?;
    let value: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
    Ok(tree(&value))
}

#[cfg(test)]
#[path = "axtree_tests.rs"]
mod tests;
