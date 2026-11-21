//! The schema.org data of a page, read once for rBrowser's first-party plugins (RB-090-96): the
//! SEO check (`plugins/seo`, the group `microdata`) and the structured data panel
//! (`plugins/structured-data`) share it instead of keeping two copies.
//!
//! - [`microdata`]: the items of a document's Microdata and RDFa Lite.
//! - [`jsonld`]: the entities of its JSON-LD blocks, scalar properties as text.
//! - [`rich`]: the rich-result table `data/rich-results.json` (the types the rules know, the
//!   properties a rich result needs and recommends).
//! - [`vocab`]: the schema.org vocabulary `data/vocabulary.json`, written by
//!   `scripts/schema-org-vocabulary.sh` from a pinned release (CC BY-SA 3.0, see `NOTICE`).
//! - [`html`] and [`path`]: the selector, text and element path helpers the readers use.

pub mod html;
pub mod jsonld;
pub mod microdata;
pub mod path;
pub mod rich;
pub mod vocab;

/// A schema.org IRI or compact name by its short name (`Product` of `https://schema.org/Product`
/// or `schema:Product`); `None` for anything with another prefix.
#[must_use]
pub fn short_name(token: &str) -> Option<&str> {
    let token = token.trim();
    for prefix in [
        "https://schema.org/",
        "http://schema.org/",
        "https://www.schema.org/",
        "http://www.schema.org/",
        "schema:",
    ] {
        if let Some(name) = token.strip_prefix(prefix) {
            return Some(name.trim_end_matches('/'));
        }
    }
    (!token.contains(':') && !token.contains('/')).then_some(token)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
