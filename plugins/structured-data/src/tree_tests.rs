use serde_json::json;

use super::*;

fn jsonld(text: &str) -> Vec<Node> {
    let mut builder = Builder::default();
    let value: serde_json::Value = serde_json::from_str(text).expect("json");
    let mut nodes = builder.json_ld(&value, 0, "html > head > script");
    finish(&mut nodes);
    nodes
}

fn values<'a>(node: &'a Node, name: &str) -> &'a [Val] {
    node.props
        .iter()
        .find(|prop| prop.name == name)
        .map(|prop| prop.values.as_slice())
        .unwrap_or_default()
}

#[test]
fn a_graph_gives_one_top_level_node_per_entry_with_nested_nodes_numbered_in_order() {
    let nodes = jsonld(
        r##"{"@context": "https://schema.org", "@graph": [
  {"@type": "Organization", "@id": "#org", "name": "Example"},
  {"@type": "Article", "headline": "Hello", "author": {"@type": "Person", "name": "Ada"},
   "publisher": {"@id": "#org"}, "wordCount": 120, "isAccessibleForFree": true}
]}"##,
    );
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].id, "n0");
    assert_eq!(nodes[0].types, ["Organization"]);
    assert_eq!(nodes[0].at_id.as_deref(), Some("#org"));
    assert_eq!(nodes[0].pointer, "/@graph/0");
    let article = &nodes[1];
    assert_eq!(article.id, "n1");
    assert_eq!(article.source.block, Some(0));
    assert_eq!(article.source.syntax, Syntax::JsonLd);
    let [Val::Node { node: author }] = values(article, "author") else {
        panic!("one author");
    };
    assert_eq!(author.id, "n2");
    assert_eq!(author.pointer, "/@graph/1/author");
    assert_eq!(
        values(article, "publisher"),
        [Val::Ref {
            at_id: "#org".to_owned(),
            target: Some("n0".to_owned())
        }]
    );
    assert_eq!(
        values(article, "wordCount"),
        [Val::Number {
            text: "120".to_owned()
        }]
    );
    assert_eq!(
        values(article, "isAccessibleForFree"),
        [Val::Bool { value: true }]
    );
}

#[test]
fn iris_lists_and_value_objects_are_read_as_schema_org_names_and_values() {
    let nodes = jsonld(
        r#"[{"@context": {"@vocab": "https://schema.org/"}, "@type": "https://schema.org/Product",
  "schema:name": "Lamp", "color": {"@list": ["red", "blue"]}, "sku": {"@value": "A-1"},
  "https://example.org/extra": "x", "image": null}]"#,
    );
    assert_eq!(nodes.len(), 1);
    let product = &nodes[0];
    assert_eq!(product.types, ["Product"]);
    assert!(product.schema);
    assert_eq!(values(product, "name").len(), 1);
    assert_eq!(values(product, "color").len(), 2);
    assert_eq!(
        values(product, "sku"),
        [Val::Text {
            text: "A-1".to_owned()
        }]
    );
    assert!(values(product, "image").is_empty());
    assert!(
        product
            .props
            .iter()
            .any(|prop| prop.name == "https://example.org/extra")
    );
}

#[test]
fn a_block_without_the_schema_org_context_keeps_its_types_apart() {
    let nodes = jsonld(r#"{"@context": "https://example.org/", "@type": "Thing", "name": "x"}"#);
    assert!(nodes[0].types.is_empty());
    assert_eq!(nodes[0].other_types, ["Thing"]);
    assert!(!nodes[0].schema);
    let full = jsonld(r#"{"@type": "https://schema.org/Thing"}"#);
    assert_eq!(full[0].types, ["Thing"]);
}

#[test]
fn a_reference_to_an_unknown_id_stays_unresolved() {
    let nodes = jsonld(
        r##"{"@context": "https://schema.org", "@type": "WebPage", "about": {"@id": "https://example.test/#thing"}}"##,
    );
    assert_eq!(
        values(&nodes[0], "about"),
        [Val::Ref {
            at_id: "https://example.test/#thing".to_owned(),
            target: None
        }]
    );
}

#[test]
fn microdata_items_become_nodes_after_the_json_ld_ones() {
    let html = scraper::Html::parse_document(
        r#"<html><body><div itemscope itemtype="https://schema.org/Product"><span itemprop="name">Lamp</span>
<div itemprop="offers" itemscope itemtype="https://schema.org/Offer"><meta itemprop="price" content="9"></div></div></body></html>"#,
    );
    let items = schema_org::microdata::items(&html);
    let mut builder = Builder::default();
    let mut nodes = jsonld_then(
        &mut builder,
        r#"{"@context": "https://schema.org", "@type": "Thing"}"#,
    );
    nodes.extend(items.iter().map(|item| builder.item(item, String::new())));
    finish(&mut nodes);
    assert_eq!(nodes.len(), 2);
    let product = &nodes[1];
    assert_eq!(product.id, "n1");
    assert_eq!(product.source.syntax, Syntax::Microdata);
    assert_eq!(product.source.path, "html > body > div");
    let [Val::Node { node: offer }] = values(product, "offers") else {
        panic!("one offer");
    };
    assert_eq!(offer.id, "n2");
    assert_eq!(offer.pointer, "/offers/0");
    assert_eq!(
        values(offer, "price"),
        [Val::Text {
            text: "9".to_owned()
        }]
    );
}

fn jsonld_then(builder: &mut Builder, text: &str) -> Vec<Node> {
    let value: serde_json::Value = serde_json::from_str(text).expect("json");
    builder.json_ld(&value, 0, "html > head > script")
}

#[test]
fn the_number_of_nodes_and_their_depth_are_bounded() {
    let many: Vec<_> = (0..MAX_NODES + 20)
        .map(|index| json!({"@type": "Thing", "name": index.to_string()}))
        .collect();
    let mut builder = Builder::default();
    let nodes = builder.json_ld(
        &json!({"@context": "https://schema.org", "@graph": many}),
        0,
        "p",
    );
    assert_eq!(nodes.len(), MAX_NODES);
    assert!(builder.truncated);

    let mut deep = json!({"@type": "Thing"});
    for _ in 0..MAX_DEPTH + 5 {
        deep = json!({"@type": "Thing", "subjectOf": deep});
    }
    let mut builder = Builder::default();
    let nodes = builder.json_ld(&json!([{"@context": "https://schema.org"}, deep]), 0, "p");
    assert!(builder.truncated);
    assert!(!nodes.is_empty());
}

#[test]
fn a_long_text_is_cut() {
    let long = "x".repeat(MAX_TEXT + 50);
    let nodes = jsonld(&format!(
        r#"{{"@context": "https://schema.org", "@type": "Thing", "description": "{long}"}}"#
    ));
    let [Val::Text { text }] = values(&nodes[0], "description") else {
        panic!("one text");
    };
    assert_eq!(text.chars().count(), MAX_TEXT);
}

#[test]
fn a_json_pointer_escapes_its_keys() {
    assert_eq!(pointer_step("a/b~c"), "a~1b~0c");
}

#[test]
fn nested_microdata_items_count_against_the_node_limit() {
    let html = scraper::Html::parse_document(
        r#"<html><body><div itemscope itemtype="https://schema.org/Product"><span itemprop="name">Lamp</span>
<div itemprop="offers" itemscope itemtype="https://schema.org/Offer"><meta itemprop="price" content="9"></div></div></body></html>"#,
    );
    let items = schema_org::microdata::items(&html);
    let many: Vec<_> = (0..MAX_NODES - 1)
        .map(|index| json!({"@type": "Thing", "name": index.to_string()}))
        .collect();
    let mut builder = Builder::default();
    let mut nodes = builder.json_ld(
        &json!({"@context": "https://schema.org", "@graph": many}),
        0,
        "p",
    );
    assert!(!builder.full());
    assert!(!builder.truncated);
    nodes.extend(items.iter().map(|item| builder.item(item, String::new())));
    assert!(builder.truncated);
    let product = nodes.last().expect("the product");
    assert_eq!(product.types, ["Product"]);
    assert!(values(product, "offers").is_empty());
    let mut count = 0;
    each(&nodes, &mut |_: &Node| count += 1);
    assert_eq!(count, MAX_NODES);
}
