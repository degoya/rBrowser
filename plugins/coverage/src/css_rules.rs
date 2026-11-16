//! The rules of a style sheet's text, for the rule list of the panel and MCP: each rule's
//! prelude (the selector, or the at-rule's head) and its range in UTF-16 units, the units CDP's
//! rule usage reports. Grouping at-rules (`@media`, `@supports`, `@container`, `@layer`,
//! `@scope`, `@starting-style`, `@document`) are descended into; every other block - a style
//! rule, `@font-face`, `@keyframes`, `@page` - is one rule. Comments and strings are skipped.
//! A rule is used when a used segment overlaps its body (`ranges::overlaps`), so a few units of
//! difference to Chromium's own range do not matter.

/// The most rules taken from one sheet.
pub const MAX_RULES: usize = 20_000;
/// The longest prelude kept, in characters.
pub const MAX_PRELUDE: usize = 200;

/// One rule of a sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    /// The selector or the at-rule's head, whitespace collapsed, at most [`MAX_PRELUDE`].
    pub prelude: String,
    /// Where the prelude starts.
    pub start: u32,
    /// After the closing brace.
    pub end: u32,
    /// After the opening brace.
    pub body: u32,
}

const GROUPING: &[&str] = &[
    "@media",
    "@supports",
    "@container",
    "@layer",
    "@scope",
    "@starting-style",
    "@document",
    "@-moz-document",
];

fn grouping(prelude: &str) -> bool {
    let head = prelude
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    GROUPING.contains(&head.as_str())
}

fn collapse(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(MAX_PRELUDE).collect()
}

struct Open {
    prelude: String,
    start: u32,
    body: u32,
    group: bool,
}

/// The rules of `text`, in source order of their end, at most [`MAX_RULES`].
#[must_use]
pub fn rules(text: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    // The prelude being read: its text and where it starts.
    let mut prelude = String::new();
    let mut prelude_start: Option<u32> = None;
    let mut at: u32 = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let here = at;
        at = at.saturating_add(u32::try_from(c.len_utf16()).unwrap_or(1));
        match c {
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                at = at.saturating_add(1);
                let mut last = '\0';
                for c in chars.by_ref() {
                    at = at.saturating_add(u32::try_from(c.len_utf16()).unwrap_or(1));
                    if last == '*' && c == '/' {
                        break;
                    }
                    last = c;
                }
            }
            '"' | '\'' => {
                prelude_start.get_or_insert(here);
                prelude.push(c);
                let mut escaped = false;
                for inner in chars.by_ref() {
                    at = at.saturating_add(u32::try_from(inner.len_utf16()).unwrap_or(1));
                    prelude.push(inner);
                    if escaped {
                        escaped = false;
                    } else if inner == '\\' {
                        escaped = true;
                    } else if inner == c || inner == '\n' {
                        break;
                    }
                }
            }
            '{' => {
                let text = collapse(&prelude);
                stack.push(Open {
                    group: grouping(&text),
                    prelude: text,
                    start: prelude_start.unwrap_or(here),
                    body: at,
                });
                prelude.clear();
                prelude_start = None;
            }
            '}' => {
                prelude.clear();
                prelude_start = None;
                if let Some(open) = stack.pop()
                    && !open.group
                    && rules.len() < MAX_RULES
                {
                    rules.push(Rule {
                        prelude: open.prelude,
                        start: open.start,
                        end: at,
                        body: open.body,
                    });
                }
            }
            ';' => {
                prelude.clear();
                prelude_start = None;
            }
            c if c.is_whitespace() && prelude_start.is_none() => {}
            c => {
                prelude_start.get_or_insert(here);
                prelude.push(c);
            }
        }
    }
    rules
}

/// The UTF-16 length of `text`.
#[must_use]
pub fn units(text: &str) -> u64 {
    text.encode_utf16().count() as u64
}

/// The part of `text` from UTF-16 unit `start` to `end` (clamped to whole characters).
#[must_use]
pub fn slice(text: &str, start: u32, end: u32) -> String {
    let mut at: u32 = 0;
    let mut out = String::new();
    for c in text.chars() {
        if at >= end {
            break;
        }
        if at >= start {
            out.push(c);
        }
        at = at.saturating_add(u32::try_from(c.len_utf16()).unwrap_or(1));
    }
    out
}

#[cfg(test)]
#[path = "css_rules_tests.rs"]
mod tests;
