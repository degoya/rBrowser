//! Ignore rules in gitignore syntax: `#` comments, `!` negation, a trailing `/` for folders, a
//! `/` anywhere else anchoring the pattern to the root, `*` within a name and `**` across
//! folders. As in Git, the last matching line wins, and a file in an ignored folder cannot be
//! taken back.
//!
//! [`DEFAULT_IGNORES`] come first, then `reload.ignore` from the config, then the lines of
//! [`IGNORE_FILE`]; so a project can take a default back with `!`.

use globset::{GlobBuilder, GlobMatcher};

/// The project's own ignore file, relative to the root (gitignore syntax).
pub const IGNORE_FILE: &str = ".rbrowser/ignore";

/// Ignored in every project: dependencies, Git, dot folders (`.git`, `.cache`, `.rbrowser`,
/// `.vscode`) and the files editors write beside the one saved (Vim's swap and test files,
/// backups, Emacs locks), which would turn a CSS save into a full reload.
pub const DEFAULT_IGNORES: &[&str] = &[
    "node_modules/",
    ".git/",
    ".*/",
    "*.swp",
    "*.swx",
    "*~",
    "4913",
    ".#*",
];

/// A line that is no valid pattern.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ignore rule {line} ({pattern:?}) is not a valid pattern: {reason}")]
pub struct RuleError {
    /// The line, counted from 1 over the lines given (comments and blank lines included).
    pub line: usize,
    /// The line as written.
    pub pattern: String,
    /// Why it does not compile.
    pub reason: String,
}

impl RuleError {
    /// The i18n key of the message a user reads.
    #[must_use]
    pub fn i18n_key(&self) -> &'static str {
        "reload.error.badRule"
    }
}

#[derive(Debug, Clone)]
struct Rule {
    matcher: GlobMatcher,
    negate: bool,
    dir_only: bool,
}

/// Compiled ignore rules.
#[derive(Debug, Clone, Default)]
pub struct IgnoreRules {
    rules: Vec<Rule>,
}

impl IgnoreRules {
    /// The rules of `lines` alone.
    ///
    /// # Errors
    ///
    /// [`RuleError`] for the first line that does not compile.
    pub fn new<I, S>(lines: I) -> Result<Self, RuleError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut rules = Vec::new();
        for (index, line) in lines.into_iter().enumerate() {
            let line = line.as_ref();
            if let Some(rule) = compile(line).map_err(|reason| RuleError {
                line: index + 1,
                pattern: line.to_owned(),
                reason,
            })? {
                rules.push(rule);
            }
        }
        Ok(Self { rules })
    }

    /// [`DEFAULT_IGNORES`] followed by `lines`.
    ///
    /// # Errors
    ///
    /// [`RuleError`] for the first of `lines` that does not compile (its line counts within
    /// `lines`).
    pub fn with_defaults<I, S>(lines: I) -> Result<Self, RuleError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut all = Self::new(DEFAULT_IGNORES.iter().copied())
            .expect("the default ignore rules compile (ignore_tests)");
        all.rules.extend(Self::new(lines)?.rules);
        Ok(all)
    }

    /// The number of rules (comments and blank lines are none).
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Whether there is no rule.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Whether `path` (relative, `/` separated) is ignored: one of its folders is, or the path
    /// itself (`is_dir` says whether it is a folder).
    #[must_use]
    pub fn is_ignored(&self, path: &str, is_dir: bool) -> bool {
        let path = path.trim_matches('/');
        if path.is_empty() {
            return false;
        }
        let mut end = 0;
        while let Some(slash) = path[end..].find('/') {
            let folder = &path[..end + slash];
            if self.decision(folder, true) == Some(true) {
                return true;
            }
            end += slash + 1;
        }
        self.decision(path, is_dir) == Some(true)
    }

    /// The verdict of the last rule matching `path` itself: `Some(true)` ignored, `Some(false)`
    /// taken back, `None` no rule matches.
    fn decision(&self, path: &str, is_dir: bool) -> Option<bool> {
        self.rules
            .iter()
            .rev()
            .find(|rule| (is_dir || !rule.dir_only) && rule.matcher.is_match(path))
            .map(|rule| !rule.negate)
    }
}

/// The pattern lines of an ignore file's `text`: without comments, blank lines and line ends.
#[must_use]
pub fn parse_ignore_file(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// One line as a rule; `None` for a comment or a blank line.
fn compile(line: &str) -> Result<Option<Rule>, String> {
    let mut pattern = line.trim_end();
    if pattern.is_empty() || pattern.starts_with('#') {
        return Ok(None);
    }
    let negate = pattern.starts_with('!');
    if negate {
        pattern = &pattern[1..];
    }
    // `\#` and `\!` stand for the character itself.
    if let Some(rest) = pattern.strip_prefix('\\')
        && (rest.starts_with('#') || rest.starts_with('!'))
    {
        pattern = rest;
    }
    let dir_only = pattern.ends_with('/');
    let pattern = pattern.trim_end_matches('/');
    let anchored = pattern.contains('/');
    let pattern = pattern.trim_start_matches('/');
    if pattern.is_empty() {
        return Ok(None);
    }
    let glob = if anchored {
        pattern.to_owned()
    } else {
        format!("**/{pattern}")
    };
    let matcher = GlobBuilder::new(&glob)
        .literal_separator(true)
        .backslash_escape(true)
        .build()
        .map_err(|error| error.kind().to_string())?
        .compile_matcher();
    Ok(Some(Rule {
        matcher,
        negate,
        dir_only,
    }))
}

#[cfg(test)]
#[path = "ignore_tests.rs"]
mod tests;
