//! RB-080-11, the group `microdata`: the page's Microdata and RDFa Lite items, read from the
//! document the essentials read (the rendered one when there is one). `microdata.type_missing`
//! (an item without a type), `microdata.type_unknown` (a schema.org type the table
//! `data/schema.json` does not know - a hint, the table holds the common types only),
//! `microdata.required_missing` (a property the type's rich result needs, from the same table,
//! nested items too) and `microdata.jsonld_conflict` (a text property the page's JSON-LD gives
//! the same entity another value).

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::audit::Page;
use crate::rules::{Element, Sink, short};

pub mod extract;
pub mod jsonld;

use extract::{Item, Value};
use jsonld::Entity;

/// The type table, compiled in.
const DATA: &str = include_str!("../../../data/schema.json");

/// What a type needs.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Required {
    pub all: Vec<String>,
    pub any: Vec<Vec<String>>,
}

/// `data/schema.json`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Table {
    pub version: u32,
    pub types: Vec<String>,
    pub required: BTreeMap<String, Required>,
}

/// The type table.
#[must_use]
pub fn table() -> Table {
    serde_json::from_str(DATA).expect("data/schema.json is the plugin's own and parses")
}

pub fn check(page: &Page<'_>, sink: &mut Sink<'_>) {
    let rules = [
        "microdata.type_missing",
        "microdata.type_unknown",
        "microdata.required_missing",
        "microdata.jsonld_conflict",
    ];
    if !rules.iter().any(|rule| sink.wants(rule)) {
        return;
    }
    let Some((html, facts)) = page.primary() else {
        return;
    };
    let items = extract::items(html);
    if items.is_empty() {
        return;
    }
    let table = table();
    for item in &items {
        held(item, &table, sink);
    }
    let entities = jsonld::entities(&facts.json_ld);
    if !entities.is_empty() {
        conflicts(&items, &entities, sink);
    }
}

/// `item` and its nested items held to the table.
fn held(item: &Item, table: &Table, sink: &mut Sink<'_>) {
    let element = || Some(Element::at(&item.path));
    let syntax = item.syntax.name().to_owned();
    if item.types.is_empty() && item.other_types.is_empty() {
        sink.emit(
            "microdata.type_missing",
            "microdata.type_missing",
            element(),
            &[("syntax", syntax.clone())],
        );
    }
    for name in &item.types {
        if !table.types.contains(name) {
            sink.emit(
                "microdata.type_unknown",
                "microdata.type_unknown",
                element(),
                &[("type", short(name, 100)), ("syntax", syntax.clone())],
            );
        }
        let Some(required) = table.required.get(name) else {
            continue;
        };
        for property in &required.all {
            if !item.props.contains_key(property) {
                sink.emit(
                    "microdata.required_missing",
                    "microdata.required_missing",
                    element(),
                    &[("type", name.clone()), ("property", property.clone())],
                );
            }
        }
        for group in &required.any {
            if !group
                .iter()
                .any(|property| item.props.contains_key(property))
            {
                sink.emit(
                    "microdata.required_missing",
                    "microdata.required_any",
                    element(),
                    &[("type", name.clone()), ("properties", group.join(", "))],
                );
            }
        }
    }
    for value in item.props.values().flatten() {
        if let Value::Item(nested) = value {
            held(nested, table, sink);
        }
    }
}

/// The JSON-LD entity that is `item`: one of its type with the same `url` (or `@id`), else the
/// only one of its type when the item is the only one of its type too.
fn same_entity<'a>(item: &Item, items: &[Item], entities: &'a [Entity]) -> Option<&'a Entity> {
    let name = item.types.first()?;
    let of_type: Vec<&Entity> = entities.iter().filter(|e| e.types.contains(name)).collect();
    if let Some(url) = item.text("url") {
        let by_url = of_type.iter().find(|entity| {
            entity.props.get("url").map(String::as_str) == Some(url)
                || entity.props.get("@id").map(String::as_str) == Some(url)
        });
        if by_url.is_some() {
            return by_url.copied();
        }
        if of_type
            .iter()
            .any(|entity| entity.props.contains_key("url"))
        {
            return None;
        }
    }
    let alone = items
        .iter()
        .filter(|other| other.types.first() == Some(name))
        .count()
        == 1;
    match of_type.as_slice() {
        [only] if alone => Some(only),
        _ => None,
    }
}

fn conflicts(items: &[Item], entities: &[Entity], sink: &mut Sink<'_>) {
    for item in items {
        let Some(entity) = same_entity(item, items, entities) else {
            continue;
        };
        for (property, values) in &item.props {
            let Some(Value::Text(ours)) = values.first() else {
                continue;
            };
            let Some(theirs) = entity.props.get(property) else {
                continue;
            };
            if ours.trim() != theirs.trim() {
                sink.emit(
                    "microdata.jsonld_conflict",
                    "microdata.jsonld_conflict",
                    Some(Element::at(&item.path)),
                    &[
                        ("type", item.types.first().cloned().unwrap_or_default()),
                        ("property", short(property, 100)),
                        ("ours", short(ours, 100)),
                        ("theirs", short(theirs, 100)),
                        ("syntax", item.syntax.name().to_owned()),
                    ],
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
