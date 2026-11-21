//! An element's path from the root, the form every check plugin shares so the host's fingerprint
//! merges the same element across plugins (re-exported by `plugins/seo/src/path.rs`, agreed with
//! `a11y-axe`): the lower-case tag names from `html` down, joined by ` > `, a step numbered
//! `:nth-of-type(n)` (1-based) only when its parent has more than one child of that tag; no ids,
//! no classes. `tests/fixtures/element-path/cases.json` holds it to the TypeScript one of every
//! plugin bundle (`plugins/web-common/src/element-path.ts`), RB-094-10.

use scraper::ElementRef;

/// The element's full path from `html`.
#[must_use]
pub fn element_path(element: ElementRef<'_>) -> String {
    let mut steps = Vec::new();
    let mut current = Some(element);
    while let Some(node) = current {
        let name = node.value().name();
        let tag = name.to_ascii_lowercase();
        let parent = node.parent().and_then(ElementRef::wrap);
        let step = match parent {
            Some(parent) => {
                let same: Vec<_> = parent
                    .child_elements()
                    .filter(|sibling| sibling.value().name() == name)
                    .collect();
                if same.len() > 1 {
                    let index = same
                        .iter()
                        .position(|sibling| sibling.id() == node.id())
                        .map_or(1, |at| at + 1);
                    format!("{tag}:nth-of-type({index})")
                } else {
                    tag
                }
            }
            None => tag,
        };
        steps.push(step);
        current = parent;
    }
    steps.reverse();
    steps.join(" > ")
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
