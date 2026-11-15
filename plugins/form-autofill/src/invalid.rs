//! Deliberately invalid values, one strategy per run or mixed: an empty required field, a wrong
//! format (an address without `@`, a value outside `pattern`), a value just outside `min`/`max` or
//! off its `step`, one character too many or too few, special characters, emoji and a very long
//! word. A strategy that does not apply to a field leaves it valid, so the run tests that strategy
//! alone. Mixed picks, per field, one of the violations the browser itself checks when the field
//! has one; `minlength`/`maxlength` only count for typed edits (a script's value never trips
//! them), so a length violation is a fallback.

use crate::bounds;
use crate::field::Field;
use crate::rng::Rng;
use crate::value::{Fill, Goal, is_text, scale};

/// How a run of `mode: invalid` breaks the fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Strategy {
    Mixed,
    Empty,
    Format,
    Bounds,
    Length,
    Chars,
}

impl Strategy {
    pub const ALL: [Strategy; 6] = [
        Strategy::Mixed,
        Strategy::Empty,
        Strategy::Format,
        Strategy::Bounds,
        Strategy::Length,
        Strategy::Chars,
    ];

    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Strategy::Mixed => "mixed",
            Strategy::Empty => "empty",
            Strategy::Format => "format",
            Strategy::Bounds => "bounds",
            Strategy::Length => "length",
            Strategy::Chars => "chars",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Strategy::ALL.into_iter().find(|s| s.id() == text)
    }
}

/// What was broken in a field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Violation {
    Empty,
    Format,
    Pattern,
    Bounds,
    Step,
    Length,
    Chars,
}

impl Violation {
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Violation::Empty => "empty",
            Violation::Format => "format",
            Violation::Pattern => "pattern",
            Violation::Bounds => "bounds",
            Violation::Step => "step",
            Violation::Length => "length",
            Violation::Chars => "chars",
        }
    }

    /// Whether the browser's constraint validation flags it for a value a script set.
    #[must_use]
    pub fn checked_by_browser(self) -> bool {
        !matches!(self, Violation::Length | Violation::Chars)
    }
}

/// Characters a form should cope with: umlauts, markup, quotes, emoji, and a 136-letter word.
fn special() -> String {
    format!(
        "\u{c4}\u{d6}\u{dc}\u{df} <b>&amp;</b> \"'\\ \u{1f600}\u{1f680} {}",
        "Supercalifragilisticexpialidocious".repeat(4)
    )
}

fn text(candidates: Vec<String>, goal: Goal) -> Fill {
    Fill::Text { candidates, goal }
}

fn format(field: &Field) -> Option<(Fill, Violation)> {
    let wrong = |values: &[&str]| {
        text(
            values.iter().map(|v| (*v).to_owned()).collect(),
            Goal::Invalid,
        )
    };
    match field.control.as_str() {
        "email" => Some((
            wrong(&["qa.example.com", "qa@", "@example.com"]),
            Violation::Format,
        )),
        "url" => Some((wrong(&["not a url", "example"]), Violation::Format)),
        _ if is_text(field) && !field.pattern.is_empty() => Some((
            wrong(&["!", "-!-", "\u{1f600}", "a", "A", "0", "a b"]),
            Violation::Pattern,
        )),
        _ => None,
    }
}

fn bounds_of(field: &Field) -> Option<(Fill, Violation)> {
    let scale = scale(field)?;
    if field.control == "range" {
        return None;
    }
    if let Some(value) = bounds::out_of_bounds(scale, &field.min, &field.max, &field.step) {
        return Some((text(vec![value], Goal::Invalid), Violation::Bounds));
    }
    if field.control == "number" {
        let value = bounds::off_step(&field.min, &field.max, &field.step)?;
        return Some((text(vec![value], Goal::Invalid), Violation::Step));
    }
    None
}

fn empty(field: &Field) -> Option<(Fill, Violation)> {
    if !field.required {
        return None;
    }
    let fill = match field.control.as_str() {
        "checkbox" => Fill::Check { checked: false },
        "radio" => Fill::Radio { value: None },
        _ if field.tag == "select" => Fill::Select {
            values: Vec::new(),
            goal: Goal::Invalid,
        },
        _ => text(vec![String::new()], Goal::Invalid),
    };
    Some((fill, Violation::Empty))
}

fn length(field: &Field) -> Option<(Fill, Violation)> {
    if !is_text(field) {
        return None;
    }
    let count = match (field.max_length, field.min_length) {
        (Some(max), _) => max as usize + 1,
        (None, Some(min)) if min > 1 => min as usize - 1,
        _ => return None,
    };
    Some((text(vec!["x".repeat(count)], Goal::Any), Violation::Length))
}

fn chars(field: &Field) -> Option<(Fill, Violation)> {
    is_text(field).then(|| (text(vec![special()], Goal::Any), Violation::Chars))
}

/// The invalid value of `field` by `strategy` (keyed by `seed` and the field), or none when the
/// strategy does not apply to it.
#[must_use]
pub fn invalid(field: &Field, strategy: Strategy, seed: u64) -> Option<(Fill, Violation)> {
    match strategy {
        Strategy::Empty => empty(field),
        Strategy::Format => format(field),
        Strategy::Bounds => bounds_of(field),
        Strategy::Length => length(field),
        Strategy::Chars => chars(field),
        Strategy::Mixed => {
            let checked: Vec<_> = [format(field), bounds_of(field), empty(field)]
                .into_iter()
                .flatten()
                .collect();
            if checked.is_empty() {
                return length(field).or_else(|| chars(field));
            }
            let mut rng = Rng::keyed(seed, &["invalid", &field.key]);
            Some(rng.pick(&checked).clone())
        }
    }
}

#[cfg(test)]
#[path = "invalid_tests.rs"]
mod tests;
