//! Which paths of a project count for hot reload: the [`IgnoreRules`] (defaults, config, the
//! ignore file) and the `reload.watch` globs. Ignored folders are not walked at all, so a watch
//! never pays for `node_modules`; the watch globs only narrow the files.
//!
//! [`IGNORE_FILE`] is the exception: it always counts (its folder `.rbrowser` is walked, the rest
//! of that folder stays ignored), so a change of the rules reaches rBrowser, which sends new ones.

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use super::WatchRules;
use super::ignore::{IGNORE_FILE, IgnoreRules, RuleError};

/// The folder of [`IGNORE_FILE`].
const IGNORE_DIR: &str = ".rbrowser";

/// The compiled [`WatchRules`].
#[derive(Debug, Clone)]
pub struct Filter {
    ignore: IgnoreRules,
    watch: Option<GlobSet>,
}

impl Filter {
    /// Compiles `rules`.
    ///
    /// # Errors
    ///
    /// [`RuleError`] for an ignore line or a watch glob that does not compile.
    pub fn new(rules: &WatchRules) -> Result<Self, RuleError> {
        let ignore = IgnoreRules::with_defaults(&rules.ignore)?;
        let watch = if rules.watch.is_empty() {
            None
        } else {
            let mut set = GlobSetBuilder::new();
            for (index, pattern) in rules.watch.iter().enumerate() {
                let glob = GlobBuilder::new(pattern.trim().trim_start_matches('/'))
                    .literal_separator(true)
                    .backslash_escape(true)
                    .build()
                    .map_err(|error| RuleError {
                        line: index + 1,
                        pattern: pattern.clone(),
                        reason: error.kind().to_string(),
                    })?;
                set.add(glob);
            }
            Some(set.build().map_err(|error| RuleError {
                line: 0,
                pattern: rules.watch.join(", "),
                reason: error.to_string(),
            })?)
        };
        Ok(Self { ignore, watch })
    }

    /// Whether a change of `path` (relative, `/` separated) counts.
    #[must_use]
    pub fn accepts(&self, path: &str, is_dir: bool) -> bool {
        if is_ignore_file(path) {
            return true;
        }
        if self.ignore.is_ignored(path, is_dir) {
            return false;
        }
        // A folder is created, removed or renamed with what is in it: it counts whenever it may
        // hold a watched file.
        is_dir || self.watch.as_ref().is_none_or(|set| set.is_match(path))
    }

    /// Whether the folder `path` is walked and watched.
    #[must_use]
    pub fn walks(&self, path: &str, is_dir: bool) -> bool {
        is_dir && (path == IGNORE_DIR || !self.ignore.is_ignored(path, true))
    }
}

/// Whether `path` is the project's ignore file.
#[must_use]
pub fn is_ignore_file(path: &str) -> bool {
    path == IGNORE_FILE
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod tests;
