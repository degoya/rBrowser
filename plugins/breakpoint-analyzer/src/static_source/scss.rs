//! SCSS and Less variables named like breakpoints - `$breakpoint-md`, `$bp-md`, `$screen-md`
//! (SCSS), `@breakpoint-md`, `@bp-md`, `@screen-sm-min` (Less) - with a length literal: the
//! weaker, heuristic source. The label is the rest of the name without `-min`; `-max` variables
//! (the pixel below another breakpoint) and values that are no literal are left out silently, as
//! the name alone does not say they are breakpoints.

use super::{FileScan, strip_comments};

/// The name prefixes read.
const PREFIXES: [&str; 3] = ["breakpoint-", "bp-", "screen-"];

/// The variables of a file whose variables start with `sigil` (`$` SCSS, `@` Less).
#[must_use]
pub fn scan(path: &str, text: &str, sigil: char, root_font_size: f64) -> FileScan {
    let mut found = FileScan::default();
    let text = strip_comments(text, true);
    for statement in text.split(';') {
        // The statement after any block it follows: `} $bp-md: 768px`.
        let statement = statement
            .rsplit(['{', '}'])
            .next()
            .unwrap_or(statement)
            .trim();
        let Some((name, value)) = statement
            .strip_prefix(sigil)
            .and_then(|rest| rest.split_once(':'))
        else {
            continue;
        };
        let name = name.trim();
        let Some(label) = PREFIXES.iter().find_map(|prefix| name.strip_prefix(prefix)) else {
            continue;
        };
        if label.is_empty()
            || label.ends_with("-max")
            || !label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            continue;
        }
        let label = label.strip_suffix("-min").unwrap_or(label);
        let value = value
            .trim()
            .trim_end_matches("!default")
            .trim_end_matches("!global")
            .trim();
        if let Some(px) = super::literal_px(value, root_font_size) {
            found.declare(path, label, px, None, true);
        }
    }
    found
}

#[cfg(test)]
#[path = "scss_tests.rs"]
mod tests;
