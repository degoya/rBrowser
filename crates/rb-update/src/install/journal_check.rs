//! What a journal may name before anything acts on it (T15, `docs/security/threat-model.md`):
//! whatever a start does with it moves program files and starts a program.
//!
//! Copied and adapted from rCommander's `crates/rc-update/src/install/journal_check.rs`
//! (RB-090-02): no database.

use std::fs;
use std::path::{Component, Path};

use super::{InstallError, Journal, portable};

impl Journal {
    /// Refuses a journal that reaches outside the installation it belongs to. On top of
    /// [`super::Plan::validate`]: the journal lies in the data directory it names; the entries
    /// and the replaced ones are plain names in the program folder, never one the switch keeps
    /// ([`portable::KEPT`]); the program is inside one of the entries.
    ///
    /// # Errors
    ///
    /// `update.journal_invalid` with what is wrong.
    pub fn check(&self, data: &Path) -> Result<(), InstallError> {
        let invalid = |detail: String| Err(InstallError::new("update.journal_invalid", detail));
        if let Err(error) = self.plan.validate() {
            return invalid(error.detail);
        }
        let plan = &self.plan;
        let canonical = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
        if canonical(&plan.data_dir) != canonical(data) {
            return invalid(format!(
                "the journal in {} names the data directory {}",
                data.display(),
                plan.data_dir.display()
            ));
        }
        if let Some(name) = self
            .entries
            .iter()
            .chain(&self.replaced)
            .find(|name| !is_name(name))
        {
            return invalid(format!("{name:?} is not a name in the program folder"));
        }
        if let Some(name) = self.entries.iter().find(|name| portable::is_kept(name)) {
            return invalid(format!("{name:?} is never replaced by an update"));
        }
        if let Some(name) = self
            .replaced
            .iter()
            .find(|name| !self.entries.contains(name))
        {
            return invalid(format!("{name:?} is replaced but no entry of the update"));
        }
        if !self.entries.is_empty()
            && !self
                .entries
                .iter()
                .any(|entry| entry == plan.program_entry())
        {
            return invalid(format!(
                "the program {} is no entry of the update",
                plan.executable
            ));
        }
        Ok(())
    }
}

/// One plain, visible name: a single normal path component that does not start with `.`, so
/// joined to a folder it stays inside it on every platform (`C:x` and `a\b` are two on Windows).
pub(crate) fn is_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':'])
        && matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
}
