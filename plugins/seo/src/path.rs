//! An element's path from the root, the form every check plugin shares so the host's fingerprint
//! merges the same element across plugins (agreed with `a11y-axe`): the lower-case tag names from
//! `html` down, joined by ` > `, a step numbered `:nth-of-type(n)` (1-based) only when its parent
//! has more than one child of that tag; no ids, no classes. `src/collect.js` builds the same
//! path in the page.

use scraper::ElementRef;

/// The element's full path from `html`.
#[must_use]
pub fn element_path(element: ElementRef<'_>) -> String {
    let mut steps = Vec::new();
    let mut current = Some(element);
    while let Some(node) = current {
        let tag = node.value().name();
        let parent = node.parent().and_then(ElementRef::wrap);
        let step = match parent {
            Some(parent) => {
                let same: Vec<_> = parent
                    .child_elements()
                    .filter(|sibling| sibling.value().name() == tag)
                    .collect();
                if same.len() > 1 {
                    let index = same
                        .iter()
                        .position(|sibling| sibling.id() == node.id())
                        .map_or(1, |at| at + 1);
                    format!("{tag}:nth-of-type({index})")
                } else {
                    tag.to_owned()
                }
            }
            None => tag.to_owned(),
        };
        steps.push(step);
        current = parent;
    }
    steps.reverse();
    steps.join(" > ")
}

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
