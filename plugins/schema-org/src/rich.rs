//! The rich-result table `data/rich-results.json` (RB-080-11, shared since RB-090-96): the
//! schema.org types the SEO check's Microdata and RDFa rules know - the common ones, not the whole
//! vocabulary - and, per rich-result type of Google Search, the properties it needs (`all`: each
//! one, `any`: at least one of each list) and the ones it recommends. One table for the SEO check
//! and the structured data panel.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The table, compiled in.
const DATA: &str = include_str!("../data/rich-results.json");

/// What a type needs.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Required {
    pub all: Vec<String>,
    pub any: Vec<Vec<String>>,
}

/// `data/rich-results.json`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Table {
    pub version: u32,
    pub types: Vec<String>,
    pub required: BTreeMap<String, Required>,
    /// The properties a rich result of the type recommends (RB-090-96); the SEO check reads none.
    #[serde(default)]
    pub recommended: BTreeMap<String, Vec<String>>,
}

/// The table.
#[must_use]
pub fn table() -> Table {
    serde_json::from_str(DATA).expect("data/rich-results.json is the crate's own and parses")
}

#[cfg(test)]
#[path = "rich_tests.rs"]
mod tests;
