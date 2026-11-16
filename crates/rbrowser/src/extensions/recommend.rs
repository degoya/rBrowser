//! A project's recommended extensions (`extensions` in its config; RB-080-06): offered only in
//! a trusted workspace with both switches on, never installed by themselves. A `path` entry is
//! a folder inside the project ([`project_folder`]); the person adds it with a command, and it
//! joins their own list of unpacked folders. A Web Store entry (`id`, `url`) is installable
//! when it names an id (RB-080-23, `super::webstore::parse_ref`) and the Web Store is allowed;
//! one that is installed already is left out.

use std::path::{Path, PathBuf};

use rb_config::{ChromiumExtensionSettings, ExtensionRecommendation, UnpackedExtension};

use super::folder::{FolderError, check, project_folder};
use super::webstore::parse_ref;

/// One recommendation as offered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Offer {
    /// A valid folder the person does not have yet.
    Folder {
        /// The path as the config writes it.
        written: String,
        /// The folder.
        folder: PathBuf,
        /// The manifest's name.
        name: String,
    },
    /// A Web Store id or URL: installable when `id` is known and the Web Store is allowed.
    Store {
        /// The id or URL as the config writes it.
        written: String,
        /// The extension id, when `written` names one.
        id: Option<String>,
    },
    /// A path that leaves the project or holds no extension rBrowser loads.
    Refused {
        /// The path as the config writes it.
        written: String,
        /// Why.
        error: FolderError,
    },
}

/// Whether a project may offer anything: trusted, extensions on, offers on.
pub fn should_offer(trusted: bool, settings: &ChromiumExtensionSettings) -> bool {
    trusted && settings.enabled && settings.offer_project_recommendations
}

/// The offers of `recommended` in the project at `root`, leaving out the folders `installed`
/// lists already (switched on or off), the Web Store extensions `store_ids` names (installed)
/// and entries that name nothing.
pub fn offers(
    root: &Path,
    recommended: &[ExtensionRecommendation],
    installed: &ChromiumExtensionSettings,
    store_ids: &[String],
) -> Vec<Offer> {
    let has = |folder: &Path| {
        let wanted = folder.to_string_lossy().to_lowercase();
        installed
            .unpacked
            .iter()
            .any(|entry| entry.path.trim().to_lowercase() == wanted)
    };
    let mut offers = Vec::new();
    for entry in recommended {
        let named = |value: &Option<String>| {
            value
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };
        if let Some(written) = named(&entry.path) {
            let checked =
                project_folder(root, &written).and_then(|folder| check(&folder.to_string_lossy()));
            match checked {
                Ok(found) if has(&found.folder) => {}
                Ok(found) => offers.push(Offer::Folder {
                    written,
                    folder: found.folder,
                    name: found.manifest.name,
                }),
                Err(error) => offers.push(Offer::Refused { written, error }),
            }
        } else if let Some(written) = named(&entry.id).or_else(|| named(&entry.url)) {
            let id = parse_ref(&written);
            if id.as_ref().is_some_and(|id| store_ids.contains(id)) {
                continue;
            }
            offers.push(Offer::Store { written, id });
        }
    }
    offers
}

/// `unpacked` with `folder` in it: its entry switched back on (letter case aside), or a new
/// switched-on entry at the end.
pub fn with_folder(unpacked: &[UnpackedExtension], folder: &str) -> Vec<UnpackedExtension> {
    let mut list = unpacked.to_vec();
    let wanted = folder.trim().to_lowercase();
    match list
        .iter_mut()
        .find(|entry| entry.path.trim().to_lowercase() == wanted)
    {
        Some(entry) => entry.enabled = true,
        None => list.push(UnpackedExtension {
            path: folder.trim().to_owned(),
            enabled: true,
        }),
    }
    list
}

/// What the notice was last shown for: the project and its offers. A config change that
/// leaves both alone shows no second notice.
pub fn fingerprint(root: &Path, offers: &[Offer]) -> String {
    format!("{}|{offers:?}", root.display())
}

#[cfg(test)]
#[path = "recommend_tests.rs"]
mod tests;
