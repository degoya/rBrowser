use serde_json::json;

use super::*;
use crate::fake::FakeHost;

fn node(id: &str, parent: Option<&str>, role: &str, name: &str, children: &[&str]) -> Value {
    let mut node = json!({
        "nodeId": id, "ignored": false, "role": { "value": role }, "name": { "value": name },
        "childIds": children, "backendDOMNodeId": id.parse::<u64>().unwrap_or(0)
    });
    if let Some(parent) = parent {
        node["parentId"] = json!(parent);
    }
    node
}

#[test]
fn the_tree_is_flat_in_order_with_depths_and_without_noise() {
    let mut hidden = node("6", Some("5"), "heading", "Hidden", &[]);
    hidden["ignored"] = json!(true);
    let answer = json!({ "nodes": [
        node("1", None, "RootWebArea", "Fixture", &["2", "5"]),
        node("2", Some("1"), "generic", "", &["3"]),
        node("3", Some("2"), "navigation", "Main", &["4"]),
        node("4", Some("3"), "StaticText", "Docs", &[]),
        node("5", Some("1"), "main", "", &["6"]),
        hidden,
    ]});
    let tree = tree(&answer);
    let lines: Vec<String> = tree
        .nodes
        .iter()
        .map(|n| format!("{}{} {}", "  ".repeat(n.depth), n.role, n.name))
        .collect();
    assert_eq!(
        lines,
        ["RootWebArea Fixture", "  navigation Main", "  main "]
    );
    assert_eq!(tree.nodes[1].backend_node_id, Some(3));
    assert!(!tree.truncated);
}

#[test]
fn a_huge_tree_is_cut_and_a_cycle_ends() {
    let mut nodes = vec![node("0", None, "RootWebArea", "", &[])];
    let children: Vec<String> = (1..=MAX_NODES + 5).map(|n| n.to_string()).collect();
    nodes[0]["childIds"] = json!(children);
    for id in &children {
        nodes.push(node(id, Some("0"), "button", "x", &[]));
    }
    let tree = tree(&json!({ "nodes": nodes }));
    assert_eq!(tree.nodes.len(), MAX_NODES);
    assert!(tree.truncated);

    let cycle = json!({ "nodes": [
        node("1", None, "RootWebArea", "", &["2"]),
        node("2", Some("1"), "group", "a", &["3"]),
        node("3", Some("2"), "group", "b", &["2"]),
    ]});
    assert!(super::tree(&cycle).nodes.len() <= 3);
}

#[test]
fn read_asks_cdp_for_the_full_tree() {
    let mut host = FakeHost::default();
    host.cdp.insert(
        "Accessibility.getFullAXTree".to_owned(),
        Ok(json!({ "nodes": [node("1", None, "RootWebArea", "Page", &[])] })),
    );
    let tree = read(&mut host, 2).expect("tree");
    assert_eq!(tree.nodes[0].name, "Page");
    assert_eq!(host.cdp_sent[0].0, 2);
}
