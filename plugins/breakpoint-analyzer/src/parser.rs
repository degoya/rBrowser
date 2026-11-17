//! The width boundaries of a stylesheet's `@media` rules (RB-010-13).
//!
//! The scan does not build a CSS tree: it skips comments, strings and escapes and reads the
//! prelude of every `@media`, `@import` and `@container` at any depth, so rules nested in other
//! at-rules or - Tailwind v4 - in style rules count as well. Each media query of a prelude is read
//! on its own (`,` and `or` separate them, `and` combines features); every width feature gives a
//! boundary:
//!
//! - `min-width: v` and `width >= v` change the layout at `ceil(v)`; `width > v` at
//!   `floor(v) + 1`;
//! - `max-width: v` and `width <= v` keep it up to `floor(v)`, so it changes at `floor(v) + 1`;
//!   `width < v` at `ceil(v)`;
//! - a feature after `not` swaps the side; `v` in `px`, `em` or `rem` (`rootFontSizePx` each).
//!
//! `@container` conditions are counted as a hint and never become breakpoints.

/// Which side of the boundary a query selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    /// `min-width` or `width >=`: from the boundary on.
    Min,
    /// `max-width` or `width <=`: up to one pixel below it.
    Max,
}

/// One width boundary found in a media query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Boundary {
    /// The first width in CSS px at which the layout differs.
    pub change: u32,
    /// The side the query selects.
    pub side: Side,
}

/// What one stylesheet holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SheetScan {
    /// Every boundary, in source order.
    pub boundaries: Vec<Boundary>,
    /// Media queries with at least one width boundary.
    pub queries: u32,
    /// `@container` rules (a hint only).
    pub containers: u32,
}

/// The widest boundary taken seriously; anything wider is a typo or a trick.
const MAX_WIDTH: f64 = 10_000.0;

/// Scans `css`; `root_font_size` converts `em` and `rem`.
#[must_use]
pub fn scan(css: &str, root_font_size: f64) -> SheetScan {
    let mut found = SheetScan::default();
    let bytes = css.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'/' if bytes.get(at + 1) == Some(&b'*') => at = skip_comment(bytes, at),
            b'"' | b'\'' => at = skip_string(bytes, at),
            b'\\' => at += 2,
            b'@' => {
                let name_end = ident_end(bytes, at + 1);
                let name = css[at + 1..name_end].to_ascii_lowercase();
                let (prelude, next) = prelude(css, name_end);
                match name.as_str() {
                    "media" | "import" => found.add_queries(prelude, root_font_size),
                    "container" => found.containers += 1,
                    _ => {}
                }
                at = next;
            }
            _ => at += 1,
        }
    }
    found
}

/// Scans a media query list on its own (an `@import`'s or a `<link media>`).
#[must_use]
pub fn scan_media_list(list: &str, root_font_size: f64) -> SheetScan {
    let mut found = SheetScan::default();
    found.add_queries(list, root_font_size);
    found
}

impl SheetScan {
    fn add_queries(&mut self, list: &str, root_font_size: f64) {
        for query in split_queries(list) {
            let boundaries = query_boundaries(query, root_font_size);
            if !boundaries.is_empty() {
                self.queries += 1;
                self.boundaries.extend(boundaries);
            }
        }
    }
}

fn skip_comment(bytes: &[u8], start: usize) -> usize {
    let mut at = start + 2;
    while at + 1 < bytes.len() && !(bytes[at] == b'*' && bytes[at + 1] == b'/') {
        at += 1;
    }
    (at + 2).min(bytes.len())
}

fn skip_string(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut at = start + 1;
    while at < bytes.len() && bytes[at] != quote && bytes[at] != b'\n' {
        at += if bytes[at] == b'\\' { 2 } else { 1 };
    }
    (at + 1).min(bytes.len())
}

fn ident_end(bytes: &[u8], start: usize) -> usize {
    let mut at = start;
    while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'-') {
        at += 1;
    }
    at
}

/// The prelude of an at-rule from `start` up to its `{` or `;` (strings and comments skipped),
/// and where scanning goes on: right after the prelude, so the block is scanned too.
fn prelude(css: &str, start: usize) -> (&str, usize) {
    let bytes = css.as_bytes();
    let mut at = start;
    while at < bytes.len() {
        match bytes[at] {
            b'{' | b';' | b'}' => return (&css[start..at], at + 1),
            b'/' if bytes.get(at + 1) == Some(&b'*') => at = skip_comment(bytes, at),
            b'"' | b'\'' => at = skip_string(bytes, at),
            _ => at += 1,
        }
    }
    (&css[start..], bytes.len())
}

/// The queries of a list, split at top-level commas and `or`.
fn split_queries(list: &str) -> Vec<&str> {
    let mut queries = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0;
    for (index, c) in list.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth <= 0 => {
                queries.push(&list[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    queries.push(&list[start..]);
    queries
}

/// The boundaries of one query: each innermost parenthesised feature, swapped after `not`.
fn query_boundaries(query: &str, root_font_size: f64) -> Vec<Boundary> {
    let lower = query.to_ascii_lowercase();
    // `not all and (min-width: 768px)`: the whole query is negated.
    let query_negated = lower.trim_start().starts_with("not ");
    let mut found = Vec::new();
    let mut open = None;
    for (index, c) in lower.char_indices() {
        match c {
            '(' => open = Some(index),
            ')' => {
                if let Some(start) = open.take() {
                    let negated = query_negated != lower[..start].trim_end().ends_with(" not");
                    let feature = &lower[start + 1..index];
                    found.extend(
                        feature_boundaries(feature, root_font_size)
                            .into_iter()
                            .map(|boundary| if negated { swapped(boundary) } else { boundary }),
                    );
                }
            }
            _ => {}
        }
    }
    found
}

/// `not (min-width: 768px)` selects up to 767 px: the same change, the other side.
fn swapped(boundary: Boundary) -> Boundary {
    let side = match boundary.side {
        Side::Min => Side::Max,
        Side::Max => Side::Min,
    };
    Boundary { side, ..boundary }
}

/// One feature: `min-width: v`, `max-width: v` or a range over `width` (`width >= v`,
/// `v <= width`, and both ends of `a <= width < b`).
fn feature_boundaries(feature: &str, root_font_size: f64) -> Vec<Boundary> {
    if let Some((name, value)) = feature.split_once(':') {
        let op = match name.trim() {
            "min-width" => ">=",
            "max-width" => "<=",
            _ => return Vec::new(),
        };
        return length(value, root_font_size)
            .and_then(|value| from_comparison(op, value))
            .into_iter()
            .collect();
    }
    let parts = split_comparisons(feature);
    let compare = |op: &str, value: &str| {
        length(value, root_font_size).and_then(|value| from_comparison(op, value))
    };
    match parts.as_slice() {
        [left, op, right] if left.trim() == "width" => compare(op, right).into_iter().collect(),
        [left, op, right] if right.trim() == "width" => {
            compare(flipped(op), left).into_iter().collect()
        }
        [low, op_low, width, op_high, high] if width.trim() == "width" => {
            compare(flipped(op_low), low)
                .into_iter()
                .chain(compare(op_high, high))
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Splits at `<=`, `>=`, `<`, `>`, `=`, keeping the operators.
fn split_comparisons(feature: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let bytes = feature.as_bytes();
    let mut start = 0;
    let mut at = 0;
    while at < bytes.len() {
        if matches!(bytes[at], b'<' | b'>' | b'=') {
            let width = if bytes.get(at + 1) == Some(&b'=') && bytes[at] != b'=' {
                2
            } else {
                1
            };
            parts.push(&feature[start..at]);
            parts.push(&feature[at..at + width]);
            at += width;
            start = at;
        } else {
            at += 1;
        }
    }
    parts.push(&feature[start..]);
    parts
}

fn flipped(op: &str) -> &'static str {
    match op {
        ">=" => "<=",
        "<=" => ">=",
        ">" => "<",
        "<" => ">",
        _ => "=",
    }
}

/// The boundary of `width <op> value`.
fn from_comparison(op: &str, value: f64) -> Option<Boundary> {
    let (change, side) = match op {
        ">=" => (value.ceil(), Side::Min),
        ">" => (value.floor() + 1.0, Side::Min),
        "<=" => (value.floor() + 1.0, Side::Max),
        "<" => (value.ceil(), Side::Max),
        _ => return None,
    };
    if !(1.0..=MAX_WIDTH).contains(&change) {
        return None;
    }
    // Bounded to 1..=10 000 above, so the cast is exact.
    Some(Boundary {
        change: change as u32,
        side,
    })
}

/// A length in CSS px: `768px`, `48em`, `40rem`; anything else is no width boundary.
pub(crate) fn length(text: &str, root_font_size: f64) -> Option<f64> {
    let text = text.trim();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(text.len());
    let number: f64 = text[..split].parse().ok()?;
    let px = match text[split..].trim() {
        "px" => number,
        "em" | "rem" => number * root_font_size,
        _ => return None,
    };
    px.is_finite().then_some(px)
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
