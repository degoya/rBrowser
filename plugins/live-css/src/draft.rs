//! The draft: rule blocks, each with its scope, and whether the views apply it.
//!
//! A block's `breakpoints` are the breakpoint ids it is meant for; none means every view. The
//! views get one stylesheet, the blocks joined in order - a scoped block inside the media query of
//! its breakpoints ([`crate::media`]). A block's braces must balance outside strings and
//! comments, so no block closes the media query around it and applies in every view.
//!
//! The limits keep a view's sheet within one page-script message (64 KiB of JSON): at most
//! [`MAX_BLOCKS`] blocks of [`MAX_CSS_BYTES`] of CSS together, no control character other than a
//! tab or a line break (a JSON escape of six bytes each).

use rb_plugin_api::{Failure, FailureKind};
use serde_json::{Value, json};

use crate::media::{Breakpoints, Scope, wrap};
use crate::{bad_args, coded};

/// The most blocks of a draft.
pub const MAX_BLOCKS: usize = 32;
/// The most breakpoints of a block.
pub const MAX_BREAKPOINTS: usize = 32;
/// The most CSS of a draft, all blocks together, in UTF-8 bytes.
pub const MAX_CSS_BYTES: usize = 30_000;
/// The longest breakpoint id.
pub const MAX_ID_CHARS: usize = 64;

/// One rule block.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Block {
    pub css: String,
    /// The breakpoint ids it applies in; none for every view.
    pub breakpoints: Vec<String>,
}

/// The draft.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    /// Whether the views apply it; off keeps the blocks.
    pub enabled: bool,
    pub blocks: Vec<Block>,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            enabled: true,
            blocks: Vec::new(),
        }
    }
}

/// Whether `id` can be a breakpoint id: 1 to 64 characters, no control character.
#[must_use]
pub fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.chars().count() <= MAX_ID_CHARS && !id.chars().any(char::is_control)
}

/// A block's `breakpoints`: none for `null`, else a list of ids, each once.
///
/// # Errors
///
/// `errors.bad_args` (`breakpoints`) for anything else.
pub fn breakpoints_from(value: &Value) -> Result<Vec<String>, Failure> {
    let items = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Array(items) if items.len() <= MAX_BREAKPOINTS => items,
        _ => return Err(bad_args("breakpoints")),
    };
    let mut ids: Vec<String> = Vec::new();
    for item in items {
        let id = item
            .as_str()
            .filter(|id| id_ok(id))
            .ok_or_else(|| bad_args("breakpoints"))?;
        if !ids.iter().any(|known| known == id) {
            ids.push(id.to_owned());
        }
    }
    Ok(ids)
}

impl Block {
    /// A block from `{"css", "breakpoints"?}`.
    ///
    /// # Errors
    ///
    /// `errors.bad_args` naming the field that does not read.
    pub fn from_json(value: &Value) -> Result<Self, Failure> {
        let css = value
            .get("css")
            .and_then(Value::as_str)
            .ok_or_else(|| bad_args("css"))?;
        let breakpoints = breakpoints_from(value.get("breakpoints").unwrap_or(&Value::Null))?;
        Ok(Self {
            css: css.to_owned(),
            breakpoints,
        })
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({ "css": self.css, "breakpoints": self.breakpoints })
    }
}

impl Draft {
    /// Holds the draft to its limits.
    ///
    /// # Errors
    ///
    /// `errors.too_many_blocks`, `errors.too_long`, and `errors.control` or `errors.braces` naming
    /// the block (from 1).
    pub fn check(&self) -> Result<(), Failure> {
        if self.blocks.len() > MAX_BLOCKS {
            return Err(
                coded(FailureKind::Limit, "errors.too_many_blocks").with_param("limit", MAX_BLOCKS)
            );
        }
        let bytes: usize = self.blocks.iter().map(|block| block.css.len()).sum();
        if bytes > MAX_CSS_BYTES {
            return Err(
                coded(FailureKind::Limit, "errors.too_long").with_param("limit", MAX_CSS_BYTES)
            );
        }
        for (index, block) in self.blocks.iter().enumerate() {
            let control = block
                .css
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r'));
            if control {
                return Err(
                    coded(FailureKind::BadData, "errors.control").with_param("block", index + 1)
                );
            }
            if !braces_balance(&block.css) {
                return Err(
                    coded(FailureKind::BadData, "errors.braces").with_param("block", index + 1)
                );
            }
        }
        Ok(())
    }

    /// The sheet the views apply: [`Draft::stylesheet`], or nothing while the draft is off.
    #[must_use]
    pub fn sheet(&self, breakpoints: &Breakpoints) -> String {
        if self.enabled {
            self.stylesheet(breakpoints)
        } else {
            String::new()
        }
    }

    /// The whole draft as one stylesheet, on or off: every block that has rules and applies
    /// somewhere, in order, a scoped one in its media query. What `draft.css` holds.
    #[must_use]
    pub fn stylesheet(&self, breakpoints: &Breakpoints) -> String {
        self.blocks
            .iter()
            .filter_map(|block| block_text(block, breakpoints))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A block as it is copied and applied: its CSS, inside its media query when it is scoped; none
/// when it has no rules or none of its breakpoints is in the grid.
#[must_use]
pub fn block_text(block: &Block, breakpoints: &Breakpoints) -> Option<String> {
    if block.css.trim().is_empty() {
        return None;
    }
    match breakpoints.scope(&block.breakpoints) {
        Scope::All => Some(block.css.clone()),
        Scope::Query(query) => Some(wrap(&query, &block.css)),
        Scope::Nowhere => None,
    }
}

/// Whether the braces of `css` balance outside strings, comments and escapes, never closing
/// more than were opened; an unclosed comment does not balance (it would swallow what follows).
#[must_use]
pub fn braces_balance(css: &str) -> bool {
    let mut depth: usize = 0;
    let mut chars = css.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut closed = false;
                while let Some(inner) = chars.next() {
                    if inner == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return false;
                }
            }
            '"' | '\'' => {
                // A string ends at its quote or, unclosed, at the line's end.
                while let Some(inner) = chars.next() {
                    match inner {
                        '\\' => {
                            chars.next();
                        }
                        '\n' => break,
                        _ if inner == c => break,
                        _ => {}
                    }
                }
            }
            '{' => depth += 1,
            '}' => match depth.checked_sub(1) {
                Some(less) => depth = less,
                None => return false,
            },
            _ => {}
        }
    }
    depth == 0
}

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;
