use super::*;

fn block(text: &str) -> JsonLd {
    JsonLd {
        path: "html > head > script".to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn graph_arrays_and_nested_objects_give_their_entities() {
    let found = entities(&[
        block(
            r##"{"@context": "https://schema.org", "@graph": [
  {"@type": "Product", "name": " Lamp\n LED ", "sku": 12, "offers": {"@type": "Offer", "price": 19.99}},
  {"@type": ["Organization", "https://schema.org/Corporation"], "@id": "#org", "name": "ACME"}
]}"##,
        ),
        block("{broken"),
    ]);
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].types, ["Product"]);
    assert_eq!(
        found[0].props.get("name").map(String::as_str),
        Some("Lamp LED")
    );
    assert_eq!(found[0].props.get("sku").map(String::as_str), Some("12"));
    assert!(!found[0].props.contains_key("offers"));
    assert_eq!(found[1].types, ["Offer"]);
    assert_eq!(
        found[1].props.get("price").map(String::as_str),
        Some("19.99")
    );
    assert_eq!(found[2].types, ["Organization", "Corporation"]);
    assert_eq!(found[2].props.get("@id").map(String::as_str), Some("#org"));
}
