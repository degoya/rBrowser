//! Bootstrap: the `$grid-breakpoints` map of the project's SCSS - set before Bootstrap is
//! imported, or passed with `@use "bootstrap" with (...)` - or Bootstrap 5's defaults when the
//! project only imports Bootstrap. `xs: 0` is no boundary; a value that is no length literal
//! (`$var`, `map-merge(...)`, `math.div(...)`) is a `dynamic` hint.

use super::{FileScan, Hint, ProjectFile, literal_px, strip_comments};
use crate::suggest::Framework;

/// Bootstrap 5's `$grid-breakpoints` (without `xs: 0`).
pub const DEFAULTS: [(&str, f64); 5] = [
    ("sm", 576.0),
    ("md", 768.0),
    ("lg", 992.0),
    ("xl", 1200.0),
    ("xxl", 1400.0),
];

const VARIABLE: &str = "$grid-breakpoints";

/// The breakpoints of the project's SCSS files.
#[must_use]
pub fn scan_project(files: &[&ProjectFile], root_font_size: f64) -> FileScan {
    let mut found = FileScan::default();
    let mut maps = 0;
    let mut importer: Option<&str> = None;
    for file in files {
        let text = strip_comments(&file.text, true);
        maps += read_maps(&file.path, &text, root_font_size, &mut found);
        if importer.is_none() && imports_bootstrap(&text) {
            importer = Some(&file.path);
        }
    }
    if maps == 0
        && let Some(path) = importer
    {
        for (label, px) in DEFAULTS {
            found.declare(path, label, px, Some(Framework::Bootstrap), false);
        }
    }
    found
}

/// Reads every assignment `$grid-breakpoints: (...)` of `text`; how many there were (an
/// unreadable one counts: the defaults are overridden by something).
fn read_maps(path: &str, text: &str, root_font_size: f64, found: &mut FileScan) -> usize {
    let mut maps = 0;
    for (at, _) in text.match_indices(VARIABLE) {
        let rest = &text[at + VARIABLE.len()..];
        // `$grid-breakpoints-foo` is another variable; a use without `:` is no assignment.
        if rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            continue;
        }
        let Some(value) = rest.trim_start().strip_prefix(':') else {
            continue;
        };
        maps += 1;
        let value = value.trim_start();
        let Some(inner) = value.strip_prefix('(').map(until_close) else {
            let name = value
                .split([';', '\n', '('])
                .next()
                .unwrap_or_default()
                .trim();
            found
                .hints
                .push(Hint::new(path, "dynamic", &format!("{VARIABLE}: {name}")));
            continue;
        };
        for entry in split_top(inner) {
            let Some((key, value)) = entry.split_once(':') else {
                continue;
            };
            let key = key.trim().trim_matches(['"', '\'']);
            let value = value.trim();
            if key.is_empty() || value == "0" {
                continue;
            }
            match literal_px(value, root_font_size) {
                Some(px) => found.declare(path, key, px, Some(Framework::Bootstrap), false),
                None => found
                    .hints
                    .push(Hint::new(path, "dynamic", &format!("{VARIABLE}.{key}"))),
            }
        }
    }
    maps
}

/// The text up to the `)` that closes an already opened `(`.
fn until_close(text: &str) -> &str {
    let mut depth = 1_usize;
    for (index, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &text[..index];
                }
            }
            _ => {}
        }
    }
    text
}

/// `text` split at the commas outside parentheses.
fn split_top(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0;
    for (index, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&text[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

/// Whether the SCSS imports Bootstrap: `@import`, `@use` or `@forward` of `bootstrap`,
/// `bootstrap/scss/...` or `~bootstrap/scss/bootstrap`.
#[must_use]
pub fn imports_bootstrap(scss: &str) -> bool {
    ["@import", "@use", "@forward"].iter().any(|rule| {
        scss.match_indices(rule).any(|(at, _)| {
            let rest = scss[at + rule.len()..].trim_start();
            let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                return false;
            };
            let target = rest[1..].split(quote).next().unwrap_or_default();
            let last = target.rsplit('/').next().unwrap_or(target);
            let last = last.trim_start_matches('_').trim_end_matches(".scss");
            last == "bootstrap" || target.contains("bootstrap/scss/")
        })
    })
}

#[cfg(test)]
#[path = "bootstrap_tests.rs"]
mod tests;
