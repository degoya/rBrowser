use serde_json::{Map, Value, json};

use super::*;

fn values(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

#[test]
fn the_message_is_the_settings_marker() {
    assert_eq!(message(), json!({ "$settings": true }));
}

#[test]
fn only_the_plugins_whose_own_values_changed_are_told() {
    let mut seen = Seen::default();
    let start = vec![
        (
            "rulers-guides".to_owned(),
            values(&[("rulers-guides.colors.guide", json!("#00a2ff"))]),
        ),
        ("peek".to_owned(), values(&[("peek.delayMs", json!(300))])),
    ];
    // The first look tells every plugin: nothing was seen before it.
    assert_eq!(
        seen.changed(start.clone()),
        vec!["rulers-guides".to_owned(), "peek".to_owned()]
    );
    // The same values again: nobody.
    assert!(seen.changed(start).is_empty());
    let later = vec![
        (
            "rulers-guides".to_owned(),
            values(&[("rulers-guides.colors.guide", json!("#ff00ff"))]),
        ),
        ("peek".to_owned(), values(&[("peek.delayMs", json!(300))])),
    ];
    assert_eq!(seen.changed(later), vec!["rulers-guides".to_owned()]);
}

#[test]
fn a_plugin_gone_and_back_counts_as_new() {
    let mut seen = Seen::default();
    let one = vec![("a".to_owned(), values(&[("a.x", json!(1))]))];
    seen.changed(one.clone());
    assert!(seen.changed(Vec::new()).is_empty());
    assert_eq!(seen.changed(one), vec!["a".to_owned()]);
}
