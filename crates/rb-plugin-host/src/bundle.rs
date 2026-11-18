//! Which bundled first-party plugins are installed (RB-091-04, after rDownloader's RD-160-05 and
//! RD-180-14): the package keeps every first-party plugin in `plugins/` beside the executable, and
//! a start loads only the ones the person chose ([`crate::Sources::chosen`]). The choice lives in
//! [`BUNDLE_FILE`] in the catalogue's folder beside the user settings.
//!
//! Only the first start adds anything on its own: a fresh installation gets the recommended set -
//! or, when the first-run wizard will ask, nothing yet: the choice is *pending* until the wizard
//! answers (its plugins, or the recommended set on Skip), and a later start that finds it still
//! pending applies the recommended set. An installation that ran before RB-091-04 - when every
//! start loaded the whole bundle - keeps all of it. Every later start leaves the choice as it is:
//! a plugin new in the bundle of a later release is offered, never installed unasked, and one the
//! person removed does not come back. `--install-all-bundled-plugins` chooses the whole bundle.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::manifest::PluginManifest;

use serde::{Deserialize, Serialize};

use crate::repository::RepositoryError;

/// The file of the choice, in the catalogue's folder (`crate::CATALOG_DIR`).
pub const BUNDLE_FILE: &str = "bundle.json";

/// The layout of [`BUNDLE_FILE`] this build writes.
const BUNDLE_FILE_VERSION: u32 = 1;

/// The bundled plugins the person chose.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleState {
    #[serde(default)]
    pub version: u32,
    /// Plugin ids.
    #[serde(default)]
    pub chosen: BTreeSet<String>,
    /// The first-run wizard has not answered yet: nothing optional is chosen until it does.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pending: bool,
}

/// What a start did with the choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleStart {
    /// A fresh installation's first start: the recommended set.
    FirstStart,
    /// The first start of an installation that ran before RB-091-04: the whole bundle, so nothing
    /// disappears.
    Kept,
    /// Every later start: the choice as it was.
    InstalledOnly,
    /// `--install-all-bundled-plugins`: the whole bundle.
    All,
    /// A fresh installation whose first-run wizard will ask: nothing chosen yet.
    Pending,
    /// A later start that found the choice still pending (the wizard closed without an answer):
    /// the recommended set.
    Recommended,
}

impl BundleState {
    /// A choice of `chosen`.
    #[must_use]
    pub fn with(chosen: BTreeSet<String>) -> Self {
        Self {
            version: BUNDLE_FILE_VERSION,
            chosen,
            pending: false,
        }
    }

    /// Nothing chosen, waiting for the first-run wizard.
    #[must_use]
    pub fn pending() -> Self {
        Self {
            pending: true,
            ..Self::with(BTreeSet::new())
        }
    }

    /// Adds the recommended part of `bundled`; the choice is no longer pending.
    pub fn apply_recommended(
        &mut self,
        bundled: &BTreeSet<String>,
        recommended: &BTreeSet<String>,
    ) {
        self.chosen
            .extend(bundled.intersection(recommended).cloned());
        self.pending = false;
    }

    /// The choice in `dir`; `None` when there is none or it does not read (logged).
    #[must_use]
    pub fn load(dir: &Path) -> Option<Self> {
        let bytes = std::fs::read(dir.join(BUNDLE_FILE)).ok()?;
        match serde_json::from_slice(&bytes) {
            Ok(state) => Some(state),
            Err(error) => {
                tracing::warn!(%error, "the choice of bundled plugins does not read");
                None
            }
        }
    }

    /// Whether `dir` holds a choice file, readable or not.
    #[must_use]
    pub fn exists(dir: &Path) -> bool {
        dir.join(BUNDLE_FILE).is_file()
    }

    /// Writes the choice into `dir`, atomically.
    ///
    /// # Errors
    ///
    /// The folder or the file cannot be written.
    pub fn save(&self, dir: &Path) -> Result<(), RepositoryError> {
        std::fs::create_dir_all(dir)?;
        let state = Self {
            version: BUNDLE_FILE_VERSION,
            ..self.clone()
        };
        let bytes = serde_json::to_vec_pretty(&state)
            .map_err(|error| RepositoryError::Io(error.to_string()))?;
        crate::repository::write_atomic(&dir.join(BUNDLE_FILE), &bytes)
    }

    /// Adds plugin `id`; a person chose, so the choice is no longer pending.
    pub fn choose(&mut self, id: &str) {
        self.chosen.insert(id.to_owned());
        self.pending = false;
    }

    /// Takes plugin `id` out: it is no longer loaded and no later start brings it back.
    pub fn unchoose(&mut self, id: &str) {
        self.chosen.remove(id);
        self.pending = false;
    }
}

/// What the packages in `dir` say of themselves, by path, read once without verifying them: the
/// bundle's ids for the choice, and the loader's declared manifests
/// ([`crate::Sources::declared`]), so a start reads each manifest once.
#[must_use]
pub fn declared_manifests(dir: &Path) -> BTreeMap<PathBuf, PluginManifest> {
    let sources = crate::Sources {
        bundled: Some(dir.to_path_buf()),
        ..crate::Sources::default()
    };
    crate::loader::discover(&sources)
        .into_iter()
        .filter_map(|(_, path)| {
            crate::loader::declared_manifest(&path).map(|manifest| (path, manifest))
        })
        .collect()
}

/// How a start chooses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StartFlags {
    /// The user settings were there before this start.
    pub ran_before: bool,
    /// `--install-all-bundled-plugins`.
    pub install_all: bool,
    /// The first-run wizard will ask at this start (`ui.firstRunWizard` on, a fresh installation).
    pub wizard: bool,
}

/// The choice a start loads with: the stored `state` (the recommended set added when it is still
/// pending), or - at the first start - all of `bundled` for an installation that `ran_before`,
/// nothing yet when the wizard will ask, and the recommended part otherwise; `install_all` adds
/// the whole bundle at any start.
#[must_use]
pub fn start(
    state: Option<BundleState>,
    bundled: &BTreeSet<String>,
    recommended: &BTreeSet<String>,
    flags: StartFlags,
) -> (BundleState, BundleStart) {
    let (mut state, mut how) = match state {
        Some(mut state) if state.pending => {
            state.apply_recommended(bundled, recommended);
            (state, BundleStart::Recommended)
        }
        Some(state) => (state, BundleStart::InstalledOnly),
        None if flags.ran_before => (BundleState::with(bundled.clone()), BundleStart::Kept),
        None if flags.wizard => (BundleState::pending(), BundleStart::Pending),
        None => {
            let mut state = BundleState::with(BTreeSet::new());
            state.apply_recommended(bundled, recommended);
            (state, BundleStart::FirstStart)
        }
    };
    if flags.install_all {
        state.chosen.extend(bundled.iter().cloned());
        state.pending = false;
        how = BundleStart::All;
    }
    (state, how)
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
