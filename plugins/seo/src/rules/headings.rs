//! `headings.*`: exactly one `h1`, and no heading level skipped on the way down (`h2` to `h4`).

use crate::audit::Page;
use crate::rules::{Element, Sink, short};

pub fn check(page: &Page<'_>, sink: &mut Sink<'_>) {
    let Some((_, facts)) = page.primary() else {
        return;
    };
    let h1: Vec<_> = facts
        .headings
        .iter()
        .filter(|heading| heading.level == 1)
        .collect();
    if h1.is_empty() {
        sink.emit("headings.h1_missing", "headings.h1_missing", None, &[]);
    }
    if h1.len() > 1 {
        for heading in &h1[1..] {
            sink.emit(
                "headings.h1_multiple",
                "headings.h1_multiple",
                Some(Element::at(&heading.path)),
                &[
                    ("count", h1.len().to_string()),
                    ("text", short(&heading.text, 120)),
                ],
            );
        }
    }
    let mut previous: Option<u8> = None;
    for heading in &facts.headings {
        if let Some(before) = previous
            && heading.level > before + 1
        {
            sink.emit(
                "headings.skipped",
                "headings.skipped",
                Some(Element::at(&heading.path)),
                &[
                    ("from", format!("h{before}")),
                    ("to", format!("h{}", heading.level)),
                    ("text", short(&heading.text, 120)),
                ],
            );
        }
        previous = Some(heading.level);
    }
}

#[cfg(test)]
#[path = "headings_tests.rs"]
mod tests;
