//! The schema.org vocabulary of one pinned release, `data/vocabulary.json`, written by
//! `scripts/schema-org-vocabulary.sh` (never by hand): every class with its supertypes, the data
//! types, the enumeration members and every property with the types it is for (`domainIncludes`),
//! the types its values are expected to be (`rangeIncludes`) and its successor (`supersededBy`).
//! The vocabulary is CC BY-SA 3.0 (`NOTICE`).

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

/// The vocabulary, compiled in.
const DATA: &str = include_str!("../data/vocabulary.json");

/// The deepest class hierarchy followed (schema.org's is below 10).
const MAX_DEPTH: usize = 32;

/// One property.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Property {
    /// The types it is for (`domainIncludes`).
    #[serde(rename = "d", default)]
    pub domains: Vec<String>,
    /// The types its values are expected to be (`rangeIncludes`).
    #[serde(rename = "r", default)]
    pub ranges: Vec<String>,
    /// The property that replaces it (`supersededBy`).
    #[serde(rename = "s", default)]
    pub superseded_by: Option<String>,
}

/// `data/vocabulary.json`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Vocabulary {
    /// The schema.org release (`30.1`).
    pub version: String,
    types: BTreeMap<String, Vec<String>>,
    data_types: BTreeSet<String>,
    members: BTreeMap<String, Vec<String>>,
    properties: BTreeMap<String, Property>,
}

impl Vocabulary {
    /// The compiled-in vocabulary.
    #[must_use]
    pub fn load() -> Self {
        serde_json::from_str(DATA).expect("data/vocabulary.json is the crate's own and parses")
    }

    /// Whether `name` is a schema.org class (data types included).
    #[must_use]
    pub fn is_type(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    /// The property `name`.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&Property> {
        self.properties.get(name)
    }

    /// `name` and every type above it.
    #[must_use]
    pub fn ancestors(&self, name: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut next = vec![(name.to_owned(), 0)];
        while let Some((current, depth)) = next.pop() {
            if depth > MAX_DEPTH || !seen.insert(current.clone()) {
                continue;
            }
            for parent in self.types.get(&current).into_iter().flatten() {
                next.push((parent.clone(), depth + 1));
            }
        }
        seen
    }

    /// Whether `name` is `of` or below it.
    #[must_use]
    pub fn is_a(&self, name: &str, of: &str) -> bool {
        self.ancestors(name).contains(of)
    }

    /// The data type `name` is or is below (`Text` for `URL`), if any.
    #[must_use]
    pub fn data_type_of(&self, name: &str) -> Option<String> {
        self.ancestors(name)
            .into_iter()
            .find(|ancestor| self.data_types.contains(ancestor))
    }

    /// Whether `name` is an enumeration (a type below `Enumeration`).
    #[must_use]
    pub fn is_enumeration(&self, name: &str) -> bool {
        name != "Enumeration" && self.is_a(name, "Enumeration")
    }

    /// The enumerations `name` is a member of (`ItemAvailability` for `InStock`).
    #[must_use]
    pub fn enumerations_of(&self, name: &str) -> &[String] {
        self.members
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Whether `property` is for one of `types` or a type above them; `true` for a property
    /// without domains.
    #[must_use]
    pub fn allows(&self, types: &[String], property: &Property) -> bool {
        property.domains.is_empty()
            || types.iter().any(|name| {
                let above = self.ancestors(name);
                property.domains.iter().any(|domain| above.contains(domain))
            })
    }
}

#[cfg(test)]
#[path = "vocab_tests.rs"]
mod tests;
