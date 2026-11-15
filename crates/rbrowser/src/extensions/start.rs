//! What a start loads: the person's switched-on unpacked folders, each checked before CEF
//! starts and handed to Chromium as `--load-extension` (the Chrome bootstrap's own loader; CEF
//! 156 has no extension API of its own any more), and the Web Store extensions installed under
//! rBrowser's local folder (RB-080-23, [`plan_with_store`]), loaded the same way. What is loaded
//! stays for the instance's life: a change takes effect at the next start ([`needs_restart`]).

use std::sync::OnceLock;

use rb_config::ChromiumExtensionSettings;

use super::folder::{FolderError, Unpacked, check};
use super::store::{ExtensionStore, InstalledExtension};

/// Chromium's switch: the folders, separated by commas.
pub const LOAD_SWITCH: &str = "load-extension";

/// The folders of one start.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartPlan {
    /// Loaded, in the list's order; the Web Store extensions' folders after the person's.
    pub loaded: Vec<Unpacked>,
    /// Switched on but refused, with the reason.
    pub skipped: Vec<Skipped>,
    /// The Web Store extensions among `loaded`.
    pub web_store: Vec<InstalledExtension>,
}

/// A folder that is not loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    /// The path as the settings give it.
    pub path: String,
    /// Why.
    pub error: FolderError,
}

impl StartPlan {
    /// The loaded folders as paths.
    pub fn loaded_paths(&self) -> Vec<&str> {
        self.loaded
            .iter()
            .filter_map(|found| found.folder.to_str())
            .collect()
    }
}

/// The plan of `settings` without the Web Store: every switched-on folder checked, a folder
/// named twice (letter case aside: Windows paths) loaded once.
pub fn plan(settings: &ChromiumExtensionSettings) -> StartPlan {
    plan_with_store(settings, None)
}

/// [`plan`], with the installed Web Store extensions of `store` after the person's folders
/// (none while the extensions are switched off).
pub fn plan_with_store(
    settings: &ChromiumExtensionSettings,
    store: Option<&ExtensionStore>,
) -> StartPlan {
    let mut plan = StartPlan::default();
    let mut seen = Vec::new();
    let mut load = |found: Unpacked| -> bool {
        let key = found.folder.to_string_lossy().to_lowercase();
        if seen.contains(&key) {
            return false;
        }
        seen.push(key);
        plan.loaded.push(found);
        true
    };
    for path in settings.enabled_paths() {
        match check(path) {
            Ok(found) => {
                load(found);
            }
            Err(error) => plan.skipped.push(Skipped {
                path: path.to_owned(),
                error,
            }),
        }
    }
    if settings.enabled
        && let Some(store) = store
    {
        for installed in store.installed() {
            let Ok(found) = check(&installed.folder.to_string_lossy()) else {
                continue;
            };
            if load(found) {
                plan.web_store.push(installed);
            }
        }
    }
    plan
}

/// [`plan_with_store`] over the default store ([`ExtensionStore::default_root`]).
pub fn current_plan(settings: &ChromiumExtensionSettings) -> StartPlan {
    let store = ExtensionStore::default_root().map(ExtensionStore::new);
    plan_with_store(settings, store.as_ref())
}

/// The browser switches of `plan`: `--load-extension=<a>,<b>`, none without a folder.
pub fn switches(plan: &StartPlan) -> Vec<(String, Option<String>)> {
    let paths = plan.loaded_paths();
    if paths.is_empty() {
        return Vec::new();
    }
    vec![(LOAD_SWITCH.to_owned(), Some(paths.join(",")))]
}

/// Whether `now` loads other folders than `started`: the change waits for a restart.
pub fn needs_restart(started: &StartPlan, now: &StartPlan) -> bool {
    let key = |plan: &StartPlan| -> Vec<String> {
        plan.loaded_paths()
            .iter()
            .map(|path| path.to_lowercase())
            .collect()
    };
    key(started) != key(now)
}

/// The plan this instance started with.
static STARTED: OnceLock<StartPlan> = OnceLock::new();

/// Keeps the plan the browser process started CEF with; once.
pub fn record(plan: StartPlan) {
    for skipped in &plan.skipped {
        tracing::warn!(
            path = %skipped.path,
            code = skipped.error.code(),
            "an extension is not loaded: {}",
            skipped.error
        );
    }
    if !plan.loaded.is_empty() {
        tracing::info!(count = plan.loaded.len(), "Chrome extensions load");
    }
    if STARTED.set(plan).is_err() {
        tracing::debug!("the extensions of the start were recorded twice");
    }
}

/// The plan this instance started with; empty before (and in tests).
pub fn started() -> StartPlan {
    STARTED.get().cloned().unwrap_or_default()
}

#[cfg(test)]
#[path = "start_tests.rs"]
mod tests;
