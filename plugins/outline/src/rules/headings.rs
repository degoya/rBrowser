//! The heading rules: a level skipped on the way down, more than one `h1`, none, a first heading
//! that is no `h1` while one follows, a heading without a name (empty, or only an image without
//! alt), and a heading longer than `outline.headingMaxLength`.

use crate::page::{PageOutline, cut};

use super::{
    H1_MISSING, HEADING_EMPTY, HEADING_FIRST_NOT_H1, HEADING_IMAGE_NO_ALT, HEADING_SKIP,
    HEADING_TOO_LONG, MULTIPLE_H1, Page, Sink, level, root,
};

/// The longest piece of a long heading a finding quotes, in characters.
const QUOTE_CHARS: usize = 80;

pub(super) fn check(sink: &mut Sink<'_>, page: Page<'_>, outline: &PageOutline, max_chars: usize) {
    let exposed: Vec<_> = outline.headings.iter().filter(|h| !h.at.hidden).collect();
    let mut previous: Option<u32> = None;
    for heading in &exposed {
        let current = level(heading.level);
        if let Some(before) = previous
            && current > before + 1
        {
            sink.emit(
                HEADING_SKIP,
                page,
                Some(&heading.at),
                &[
                    ("from", format!("h{before}")),
                    ("to", format!("h{current}")),
                    ("text", heading.text.clone()),
                ],
            );
        }
        previous = Some(current);
    }
    let h1: Vec<_> = exposed.iter().filter(|h| level(h.level) == 1).collect();
    if h1.len() > 1 {
        for heading in h1.iter().skip(1) {
            sink.emit(
                MULTIPLE_H1,
                page,
                Some(&heading.at),
                &[
                    ("count", h1.len().to_string()),
                    ("text", heading.text.clone()),
                ],
            );
        }
    }
    if h1.is_empty() {
        sink.emit(H1_MISSING, page, Some(&root()), &[]);
    } else if let Some(first) = exposed.first()
        && level(first.level) != 1
    {
        sink.emit(
            HEADING_FIRST_NOT_H1,
            page,
            Some(&first.at),
            &[
                ("level", format!("h{}", level(first.level))),
                ("text", first.text.clone()),
            ],
        );
    }
    for heading in &exposed {
        if heading.text.is_empty() && heading.name.is_empty() {
            let id = if heading.image {
                HEADING_IMAGE_NO_ALT
            } else {
                HEADING_EMPTY
            };
            let at_level = format!("h{}", level(heading.level));
            sink.emit(id, page, Some(&heading.at), &[("level", at_level)]);
        }
        let length = usize::try_from(heading.length)
            .unwrap_or(0)
            .max(heading.text.chars().count());
        if length > max_chars {
            sink.emit(
                HEADING_TOO_LONG,
                page,
                Some(&heading.at),
                &[
                    ("length", length.to_string()),
                    ("max", max_chars.to_string()),
                    ("text", cut(&heading.text, QUOTE_CHARS)),
                ],
            );
        }
    }
}

#[cfg(test)]
#[path = "headings_tests.rs"]
mod tests;
