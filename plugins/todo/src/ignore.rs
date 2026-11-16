//! The folders a project's `.gitignore` files leave out (RB-110-16): the search for todo files
//! does not go into them. Only folders are judged - a todo file git ignores (a personal
//! `TODO.todo`) is still found when its folder is searched.
//!
//! What is read of a `.gitignore`, as git reads it: a comment (`#`) and a blank line are no
//! rules, `!` turns a rule into a negation, a trailing `/` is dropped (only folders are judged
//! here), a rule with a `/` before its end is a path from the file's folder (a leading `/`
//! dropped), one without names a folder at any depth below; `**`, `*` and `?` as in
//! [`crate::glob`]; within a file the last rule that matches decides, a deeper file overrules a
//! higher one. A rule this reading does not understand (an escape `\`, a class `[...]`) never
//! leaves out more than git would: as an exclusion it matches nothing, as a negation every
//! folder.

use std::rc::Rc;

use crate::glob;

/// The name of the files read.
pub const FILE: &str = ".gitignore";
/// The most rules kept of one file.
pub const MAX_RULES: usize = 500;
/// The largest `.gitignore` read, in bytes; a larger one is not used.
pub const MAX_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Pattern {
    /// No `/`: a folder of this name at any depth.
    Name(String),
    /// A path from the file's folder.
    Path(String),
    /// A negation not understood: it keeps every folder.
    Any,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rule {
    negated: bool,
    pattern: Pattern,
}

/// The rules of one `.gitignore` and the folder it lies in (`""` the root).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ignore {
    base: String,
    rules: Vec<Rule>,
}

fn rule(line: &str) -> Option<Rule> {
    let line = line.trim_end_matches(['\r', ' ', '\t']);
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (negated, line) = match line.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    if line.contains(['\\', '[']) {
        return negated.then_some(Rule {
            negated,
            pattern: Pattern::Any,
        });
    }
    let line = line.trim_end_matches('/');
    let pattern = if line.contains('/') {
        Pattern::Path(line.trim_start_matches('/').to_owned())
    } else {
        Pattern::Name(line.to_owned())
    };
    match &pattern {
        Pattern::Name(text) | Pattern::Path(text) if text.is_empty() => None,
        _ => Some(Rule { negated, pattern }),
    }
}

impl Ignore {
    /// The rules of `text`, a `.gitignore` in folder `base`; `None` when it has none.
    #[must_use]
    pub fn parse(base: &str, text: &str) -> Option<Self> {
        let rules: Vec<Rule> = text.lines().filter_map(rule).take(MAX_RULES).collect();
        (!rules.is_empty()).then(|| Self {
            base: base.to_owned(),
            rules,
        })
    }

    /// What this file says of folder `path` (project-relative): `Some(true)` left out,
    /// `Some(false)` kept by a negation, `None` no rule matches.
    fn decide(&self, path: &str) -> Option<bool> {
        let relative = if self.base.is_empty() {
            path
        } else {
            path.strip_prefix(self.base.as_str())?.strip_prefix('/')?
        };
        if relative.is_empty() {
            return None;
        }
        let name = relative.rsplit('/').next().unwrap_or(relative);
        self.rules
            .iter()
            .rev()
            .find(|rule| match &rule.pattern {
                Pattern::Name(pattern) => glob::matches(pattern, name),
                Pattern::Path(pattern) => glob::matches(pattern, relative),
                Pattern::Any => true,
            })
            .map(|rule| !rule.negated)
    }
}

/// Whether the `.gitignore` files `ignores` (the root's first, the deepest last) leave out the
/// folder `path`.
#[must_use]
pub fn ignored(ignores: &[Rc<Ignore>], path: &str) -> bool {
    ignores
        .iter()
        .rev()
        .find_map(|ignore| ignore.decide(path))
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "ignore_tests.rs"]
mod tests;
