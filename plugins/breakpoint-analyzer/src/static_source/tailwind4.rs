//! Tailwind v4: the `--breakpoint-*` variables of `@theme { ... }` in the project's CSS.
//!
//! A CSS file that imports `tailwindcss` (or `tailwindcss/theme`) is an entry: it brings
//! Tailwind's default breakpoints (`sm` 40rem to `2xl` 96rem). The `@theme` blocks of the entries,
//! then those of the other CSS files, change them in order: `--breakpoint-<name>: <length>` sets
//! one, `--breakpoint-<name>: initial` removes it, `--breakpoint-*: initial` (or `--*: initial`)
//! removes them all. A value that is no length literal (`var()`, `calc()`) is a `dynamic` hint.

use super::{FileScan, Hint, ProjectFile, literal_px, strip_comments};
use crate::suggest::Framework;

/// Tailwind v4's default breakpoints, in `rem`.
pub const DEFAULTS: [(&str, f64); 5] = [
    ("sm", 40.0),
    ("md", 48.0),
    ("lg", 64.0),
    ("xl", 80.0),
    ("2xl", 96.0),
];

/// The breakpoints of the project's CSS files.
#[must_use]
pub fn scan_project(files: &[&ProjectFile], root_font_size: f64) -> FileScan {
    let stripped: Vec<(&str, String)> = files
        .iter()
        .map(|file| (file.path.as_str(), strip_comments(&file.text, false)))
        .collect();
    let (entries, others): (Vec<_>, Vec<_>) = stripped
        .iter()
        .partition(|(_, text)| imports_tailwind(text));
    let themed: Vec<&(&str, String)> = others
        .into_iter()
        .filter(|(_, text)| !theme_blocks(text).is_empty())
        .collect();
    let mut found = FileScan::default();
    // (name, px, path), in the order they were set.
    let mut state: Vec<(String, f64, String)> = Vec::new();
    if let Some((path, _)) = entries.first() {
        state = DEFAULTS
            .iter()
            .map(|(name, rem)| ((*name).to_owned(), rem * root_font_size, (*path).to_owned()))
            .collect();
    }
    for (path, text) in entries.iter().copied().chain(themed) {
        for block in theme_blocks(text) {
            for (name, value) in declarations(block) {
                apply(&mut state, &mut found, path, &name, &value, root_font_size);
            }
        }
    }
    for (name, px, path) in state {
        found.declare(&path, &name, px, Some(Framework::Tailwind), false);
    }
    found
}

fn apply(
    state: &mut Vec<(String, f64, String)>,
    found: &mut FileScan,
    path: &str,
    name: &str,
    value: &str,
    root_font_size: f64,
) {
    let initial = value.eq_ignore_ascii_case("initial");
    if initial && (name == "--*" || name == "--breakpoint-*") {
        state.clear();
        return;
    }
    let Some(breakpoint) = name.strip_prefix("--breakpoint-") else {
        return;
    };
    if breakpoint.is_empty() || breakpoint.contains('*') {
        return;
    }
    state.retain(|(known, _, _)| known != breakpoint);
    if initial {
        return;
    }
    match literal_px(value, root_font_size) {
        Some(px) => state.push((breakpoint.to_owned(), px, path.to_owned())),
        None => found.hints.push(Hint::new(path, "dynamic", name)),
    }
}

/// Whether the CSS imports Tailwind (`@import "tailwindcss"`, `@import 'tailwindcss/theme'`).
#[must_use]
pub fn imports_tailwind(css: &str) -> bool {
    css.match_indices("@import").any(|(at, _)| {
        let rest = css[at + "@import".len()..].trim_start();
        let rest = rest.strip_prefix("url(").unwrap_or(rest).trim_start();
        let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            return false;
        };
        let inner = &rest[1..];
        let target = inner.split(quote).next().unwrap_or_default();
        target == "tailwindcss" || target.starts_with("tailwindcss/theme")
    })
}

/// The contents of every `@theme` block (`@theme`, `@theme inline`, `@theme static { ... }`).
fn theme_blocks(css: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut from = 0;
    while let Some(found) = css[from..].find("@theme") {
        let start = from + found + "@theme".len();
        from = start;
        // `@theme` itself, not `@themes` or `@theme-foo`.
        if css[start..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            continue;
        }
        let Some(open) = css[start..].find(['{', ';']) else {
            break;
        };
        let open = start + open;
        if css.as_bytes()[open] == b';' {
            from = open + 1;
            continue;
        }
        let mut depth = 0_usize;
        let mut close = css.len();
        for (index, byte) in css.as_bytes()[open..].iter().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + index;
                        break;
                    }
                }
                _ => {}
            }
        }
        blocks.push(&css[open + 1..close]);
        from = close.min(css.len());
    }
    blocks
}

/// The `--name: value` declarations of a block, in order; nested blocks are left out.
fn declarations(block: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0;
    for (index, c) in block.char_indices() {
        match c {
            '(' | '{' => depth += 1,
            ')' | '}' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => {
                push_declaration(&block[start..index], &mut found);
                start = index + 1;
            }
            _ => {}
        }
    }
    push_declaration(&block[start..], &mut found);
    found
}

fn push_declaration(text: &str, found: &mut Vec<(String, String)>) {
    if let Some((name, value)) = text.split_once(':') {
        let name = name.trim();
        if name.starts_with("--") && !name.contains(['{', '}']) {
            let value = value.trim().trim_end_matches("!important").trim();
            found.push((name.to_owned(), value.to_owned()));
        }
    }
}

#[cfg(test)]
#[path = "tailwind4_tests.rs"]
mod tests;
