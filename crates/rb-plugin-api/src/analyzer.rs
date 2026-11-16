//! The native side of the `analyzer-plugin` world: `interface analyzer`.
//!
//! An analyzer proposes config changes - the `breakpoint-analyzer` (RB-010-13) proposes
//! breakpoints - and a person applies them; a plugin never writes the project config itself.

use crate::{Failure, PluginHost, Proposal};

/// The most proposals one `analyze` may answer.
pub const MAX_PROPOSALS: usize = 1_000;

/// The logic of an analyzer plugin. A store lives for one call.
pub trait Analyzer {
    /// The proposals for `args` (JSON).
    ///
    /// # Errors
    ///
    /// The analysis failed.
    fn analyze(&mut self, host: &mut dyn PluginHost, args: &str) -> Result<Vec<Proposal>, Failure>;
}

/// Whether `pointer` is a JSON Pointer a proposal may name: empty (the whole document) or `/`
/// followed by segments without unescaped `~`.
#[must_use]
pub fn valid_pointer(pointer: &str) -> bool {
    if pointer.is_empty() {
        return true;
    }
    let Some(rest) = pointer.strip_prefix('/') else {
        return false;
    };
    let mut chars = rest.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '~' && !matches!(chars.peek(), Some('0' | '1')) {
            return false;
        }
        if c.is_control() {
            return false;
        }
    }
    true
}

#[cfg(test)]
#[path = "analyzer_tests.rs"]
mod tests;
