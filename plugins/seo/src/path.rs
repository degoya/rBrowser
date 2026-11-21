//! An element's path from the root, the form every check plugin shares so the host's fingerprint
//! merges the same element across plugins (agreed with `a11y-axe`): the lower-case tag names from
//! `html` down, joined by ` > `, a step numbered `:nth-of-type(n)` (1-based) only when its parent
//! has more than one child of that tag; no ids, no classes. `schema-org` builds it
//! (`tests/fixtures/element-path/cases.json`); `src/collect.js` builds the same path in the page.

use scraper::ElementRef;

/// The element's full path from `html`: the one of `schema-org`, which both plugins share.
pub use schema_org::path::element_path;

/// A selector that finds the element: `#id` when the id is a plain CSS identifier, the path
/// otherwise.
#[must_use]
pub fn selector_of(element: ElementRef<'_>) -> String {
    match element.value().id() {
        Some(id)
            if !id.is_empty()
                && !id.starts_with(|c: char| c.is_ascii_digit() || c == '-')
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') =>
        {
            format!("#{id}")
        }
        _ => element_path(element),
    }
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
