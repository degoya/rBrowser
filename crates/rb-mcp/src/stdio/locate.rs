//! Which running rBrowser `rbrowser mcp` talks to (RB-030-02): the project's instance file by
//! its key, else any live instance whose project is the same folder (a root written with
//! `\\wsl$\` is the one written with `\\wsl.localhost\`), else - without a project - the one
//! live instance there is. Every candidate passes [`instance::probe`] first.

use std::path::{Path, PathBuf};

use super::StdioError;
use crate::instance::{self, Instance};

/// Where the bridge looks for its instance; asked again whenever the instance it had is gone.
#[derive(Clone, Debug)]
pub struct Locator {
    local: PathBuf,
    key: Option<String>,
    project: Option<String>,
}

impl Locator {
    /// Looks under `local` (`rb_config::local_dir`) for the project with `key` (the hash of
    /// its root) at `project` (the root as the caller wrote it), or for any project.
    #[must_use]
    pub fn new(local: &Path, key: Option<&str>, project: Option<&str>) -> Self {
        Self {
            local: local.to_path_buf(),
            key: key.map(str::to_owned),
            project: project.map(str::to_owned),
        }
    }

    /// The project as messages name it: `` `<root>` `` or `any project`.
    #[must_use]
    pub fn describe(&self) -> String {
        self.project
            .as_deref()
            .map_or_else(|| "any project".to_owned(), |root| format!("`{root}`"))
    }

    /// The live instance.
    ///
    /// # Errors
    ///
    /// [`StdioError::NoInstance`], or [`StdioError::Several`] when no project was given and
    /// more than one instance runs.
    pub fn find(&self) -> Result<Instance, StdioError> {
        let no_instance = || StdioError::NoInstance(self.describe());
        if let Some(found) = self
            .key
            .as_deref()
            .and_then(|key| instance::read(&instance::file_path(&self.local, Some(key))))
            .filter(instance::probe)
        {
            return Ok(found);
        }
        let files = instance::read_all(&instance::instances_dir(&self.local));
        if let Some(project) = &self.project {
            return files
                .into_iter()
                .map(|(_, instance)| instance)
                .filter(|instance| same_root(&instance.project, project))
                .find(instance::probe)
                .ok_or_else(no_instance);
        }
        if self.key.is_some() {
            return Err(no_instance());
        }
        let live: Vec<Instance> = files
            .into_iter()
            .map(|(_, instance)| instance)
            .filter(instance::probe)
            .collect();
        match <[Instance; 1]>::try_from(live) {
            Ok([one]) => Ok(one),
            Err(live) if live.is_empty() => Err(no_instance()),
            Err(live) => Err(StdioError::Several(
                live.iter()
                    .map(|instance| instance.project.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            )),
        }
    }
}

/// Whether two written roots name one folder: separators, a trailing separator, case, the
/// `\\?\UNC\` prefix and the WSL share's two host names (`wsl$`, `wsl.localhost`) aside. An
/// empty root (an instance without a project) names none.
#[must_use]
pub fn same_root(a: &str, b: &str) -> bool {
    let (a, b) = (normalized(a), normalized(b));
    !a.is_empty() && a == b
}

fn normalized(root: &str) -> String {
    let text = root.replace('/', "\\").to_lowercase();
    let text = match text.strip_prefix(r"\\?\unc\") {
        Some(rest) => format!(r"\\{rest}"),
        None => text,
    };
    let text = match text.strip_prefix(r"\\wsl$\") {
        Some(rest) => format!(r"\\wsl.localhost\{rest}"),
        None => text,
    };
    text.trim_end_matches('\\').to_owned()
}

#[cfg(test)]
#[path = "locate_tests.rs"]
mod tests;
