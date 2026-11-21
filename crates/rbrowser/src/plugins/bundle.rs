//! The bundled first-party plugins in the app (RB-091-04, after rDownloader's RD-160-05,
//! RD-1260-02 and RD-180-14): the package keeps every first-party plugin in `plugins/` beside the
//! executable - the offline way - and a start loads only the ones the person chose. The choice is
//! `plugin-catalog/bundle.json` beside the user settings ([`rb_plugin_host::BundleState`]):
//!
//! - the first start of a fresh installation chooses the recommended set
//!   ([`super::first_party::recommended`]); the first start of an installation that ran before -
//!   when every start loaded the whole bundle - keeps all of it, so nothing disappears;
//! - every later start keeps the choice: a plugin new in a later bundle is offered, never
//!   installed unasked, and one the person removed does not come back;
//! - `--install-all-bundled-plugins` ([`record_start`]) chooses the whole bundle.
//!
//! The first-run wizard and Settings > Plugins > Catalog change the choice
//! ([`super::bundle_install`]). RB-091-08: when the very first start opens the wizard, the
//! choice is written pending at once and the one load of the plugins waits for the wizard's
//! answer ([`hold_for_wizard`]), so the plugins chosen there load live, without a restart. A project config that names a plugin that is not installed gets a
//! notice with an Install button ([`notice_missing`], `plugins.catalog.missingNotice`), and MCP's
//! `plugins_list` names it ([`missing_now`]); MCP never installs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use rb_config::ProjectConfig;
use rb_ipc::{Notification, NotificationLevel, StatusAction};
use rb_plugin_host::{BundleStart, BundleStartFlags, BundleState, PluginManifest, bundle_start};

use crate::control::Core;

/// The status bar's notice of plugins the project names that are not installed (`{plugins}`).
pub const MISSING_NOTICE_KEY: &str = "plugins.notice.missing";
/// Its button, which opens Settings > Plugins > Catalog.
pub const MISSING_ACTION_KEY: &str = "plugins.notice.missing_install";
/// The status bar's notice of the bundled plugins new in this version (`{count}`, `{plugins}`,
/// RB-094-13), named once after the update that brought them.
pub const NEW_NOTICE_KEY: &str = "plugins.notice.new_in_bundle";
/// Its button, which installs them (`plugins.install` with `bundled`).
pub const NEW_ACTION_KEY: &str = "plugins.notice.new_install";

/// How this process was started.
static START: OnceLock<BundleStartFlags> = OnceLock::new();
/// The bundled plugins this start chose, the base of a change when the choice file cannot be
/// read (review 4).
static CHOSEN_AT_START: OnceLock<BTreeSet<String>> = OnceLock::new();
/// The bundled plugins new in this version, until their notice takes them (RB-094-13).
static NEW_IN_BUNDLE: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// Records how the app was started, before anything loads a plugin: `install_all` for
/// `--install-all-bundled-plugins`, `ran_before` when the user settings were there already,
/// `wizard` when `ui.firstRunWizard` is on - on a fresh installation the wizard then asks, and the
/// choice waits for it (review 7).
pub fn record_start(install_all: bool, ran_before: bool, wizard: bool) {
    let _ = START.set(BundleStartFlags {
        ran_before,
        install_all,
        wizard: wizard && !ran_before,
    });
}

/// One writer of the choice at a time.
fn writing() -> MutexGuard<'static, ()> {
    static WRITE: Mutex<()> = Mutex::new(());
    rb_util::lock(&WRITE)
}

/// The bundled plugins this start loads, of the bundle whose manifests are `bundled` (read once,
/// review 8); `None` - every one - without a folder for the choice.
pub fn chosen_for_load(bundled: &BTreeMap<PathBuf, PluginManifest>) -> Option<BTreeSet<String>> {
    let dir = super::catalog::store_dir()?;
    let flags = start_flags();
    let ids: BTreeSet<String> = bundled
        .values()
        .map(|manifest| manifest.id.clone())
        .collect();
    let chosen = chosen_at_start(&dir, &ids, &super::first_party::recommended(), flags);
    let _ = CHOSEN_AT_START.set(chosen.clone());
    Some(chosen)
}

/// The choice in `dir` a start loads with, written back when the start made it.
pub fn chosen_at_start(
    dir: &Path,
    bundled: &BTreeSet<String>,
    recommended: &BTreeSet<String>,
    flags: BundleStartFlags,
) -> BTreeSet<String> {
    let _guard = writing();
    // A choice file that does not read counts as an installation that ran before.
    let flags = BundleStartFlags {
        ran_before: flags.ran_before || BundleState::exists(dir),
        ..flags
    };
    let (mut state, how) = bundle_start(BundleState::load(dir), bundled, recommended, flags);
    // RB-094-13: a plugin new in this bundle is named once - kept as offered from now on.
    let new = state.new_in(bundled);
    state.offered.extend(new.iter().cloned());
    if how != BundleStart::InstalledOnly || !new.is_empty() {
        if let Err(error) = state.save(dir) {
            tracing::warn!(
                code = error.code(),
                "the choice of bundled plugins was not saved"
            );
        }
        tracing::info!(
            start = ?how,
            chosen = state.chosen.len(),
            bundled = bundled.len(),
            "plugins: the bundled plugins chosen at this start"
        );
    }
    if !new.is_empty() {
        tracing::info!(new = ?new, "plugins: bundled plugins new in this version, named once");
        rb_util::lock(&NEW_IN_BUNDLE).extend(new);
    }
    state.chosen
}

/// Takes the bundled plugins new in this version that were not named yet.
pub fn take_new_in_bundle() -> BTreeSet<String> {
    std::mem::take(&mut *rb_util::lock(&NEW_IN_BUNDLE))
}

/// Changes the choice in `dir` with `change` and saves it. A choice file that cannot be read is
/// replaced by `loaded` - what this start loads - changed, never by the change alone: one install
/// must not shrink the choice to that plugin (review 4).
///
/// # Errors
///
/// The code of a choice that was not saved.
pub fn update_choice_in(
    dir: &Path,
    loaded: &BTreeSet<String>,
    change: impl FnOnce(&mut BundleState),
) -> Result<BundleState, String> {
    let _guard = writing();
    let mut state = BundleState::load(dir).unwrap_or_else(|| BundleState::with(loaded.clone()));
    change(&mut state);
    state.save(dir).map_err(|error| error.code().to_owned())?;
    Ok(state)
}

/// [`update_choice_in`] the catalogue's folder, from what this start chose.
///
/// # Errors
///
/// As [`update_choice_in`]; `plugins.catalog.error.io` without a folder.
pub fn update_choice(change: impl FnOnce(&mut BundleState)) -> Result<BundleState, String> {
    let dir = super::catalog::store_dir().ok_or_else(|| "plugins.catalog.error.io".to_owned())?;
    let loaded = CHOSEN_AT_START
        .get()
        .cloned()
        .or_else(installed_ids)
        .unwrap_or_default();
    update_choice_in(&dir, &loaded, change)
}

/// How this process was started; an unrecorded one (not the app) as an installation that ran
/// before, so nothing a person relies on goes missing.
fn start_flags() -> BundleStartFlags {
    START.get().copied().unwrap_or(BundleStartFlags {
        ran_before: true,
        install_all: false,
        wizard: false,
    })
}

/// Whether the first choice waits for the first-run wizard (RB-091-08): a fresh installation
/// whose wizard asks (`flags.wizard`), without `--install-all-bundled-plugins`, and no choice
/// yet. Anything else loads as before.
#[must_use]
pub fn should_hold(flags: BundleStartFlags, choice_exists: bool) -> bool {
    flags.wizard && !flags.install_all && !choice_exists
}

/// The very first start opens the first-run wizard: when the choice is to wait for it
/// ([`should_hold`]), the one load of the plugins is held until the plugin step answers it or the
/// wizard closes, and the choice is written pending - what the load would write. A load that began
/// already (an early caller) refuses the hold, so the answer says "next start", never "live"; a
/// choice that cannot be written lets go of the hold, and the load chooses as before.
pub fn hold_for_wizard() {
    let Some(dir) = super::catalog::store_dir() else {
        return;
    };
    let _guard = writing();
    if !should_hold(start_flags(), BundleState::exists(&dir)) || !crate::analyzer::plugins::hold() {
        return;
    }
    // The wizard offers the whole bundle: none of it is new after the answer (RB-094-13).
    let mut pending = BundleState::pending();
    pending.offered = super::bundle_offer::bundled_plugins().into_keys().collect();
    if let Err(error) = pending.save(&dir) {
        tracing::warn!(
            code = error.code(),
            "the pending plugin choice was not saved; the plugins load now"
        );
        crate::analyzer::plugins::release();
    }
}

/// Whether the choice waits for the first-run wizard's answer (review 7).
pub fn choice_pending() -> bool {
    super::catalog::store_dir()
        .and_then(|dir| BundleState::load(&dir))
        .is_some_and(|state| state.pending)
}

/// The first-party plugins `config` names: a top-level section or a `checks` entry under a
/// first-party plugin's id (its settings).
pub fn named_plugins(config: &ProjectConfig) -> BTreeSet<String> {
    config
        .extra
        .keys()
        .chain(config.checks.extra.keys())
        .filter(|id| super::first_party::is_first_party(id))
        .cloned()
        .collect()
}

/// Of `named`, the ones not in `installed`, in order.
pub fn missing(named: &BTreeSet<String>, installed: &BTreeSet<String>) -> Vec<String> {
    named.difference(installed).cloned().collect()
}

/// The ids the load gave - loaded or switched off - once it is done; `None` while it runs.
pub fn installed_ids() -> Option<BTreeSet<String>> {
    let loaded = crate::analyzer::plugins::loaded()?;
    let report = &loaded.report;
    Some(
        report
            .loaded
            .iter()
            .map(|entry| entry.plugin.manifest().id.clone())
            .chain(
                report
                    .disabled
                    .iter()
                    .map(|entry| entry.manifest.id.clone()),
            )
            .collect(),
    )
}

/// The plugins `core`'s project names that are not installed, once the load is done.
pub fn missing_now(core: &Core) -> Vec<String> {
    let Some(installed) = installed_ids() else {
        return Vec::new();
    };
    missing(&named_plugins(&core.host().config()), &installed)
}

/// Says once per set in the status bar which plugins the project names that are not installed,
/// with a button to the catalogue (`plugins.catalog.missingNotice`). Never waits for the load.
pub fn notice_missing(core: &Core) {
    static LAST: Mutex<Vec<String>> = Mutex::new(Vec::new());
    if !core.user_settings().plugins.catalog.missing_notice {
        return;
    }
    let missing = missing_now(core);
    {
        let mut last = rb_util::lock(&LAST);
        if missing.is_empty() || *last == missing {
            return;
        }
        last.clone_from(&missing);
    }
    let notice = Notification {
        level: NotificationLevel::Info,
        message_key: MISSING_NOTICE_KEY.to_owned(),
        args: [("plugins".to_owned(), missing.join(", "))].into(),
        settings_key: Some(super::catalog::CATALOG_SETTING.to_owned()),
    };
    let action = StatusAction {
        command: "plugins.catalog".to_owned(),
        label_key: MISSING_ACTION_KEY.to_owned(),
        label_args: Default::default(),
        args: serde_json::Value::Null,
    };
    core.notify_with_action(notice, action);
}

/// Names the bundled plugins new in this version once in the status bar, with a button that
/// installs them and the catalogue as the notice's setting (RB-094-13,
/// `plugins.catalog.newNotice`). Never waits for the load.
pub fn notice_new(core: &Core) {
    let new = take_new_in_bundle();
    if new.is_empty() || !core.user_settings().plugins.catalog.new_notice {
        return;
    }
    let ids: Vec<String> = new.into_iter().collect();
    let notice = Notification {
        level: NotificationLevel::Info,
        message_key: NEW_NOTICE_KEY.to_owned(),
        args: [
            ("count".to_owned(), ids.len().to_string()),
            ("plugins".to_owned(), ids.join(", ")),
        ]
        .into(),
        settings_key: Some(super::catalog::CATALOG_SETTING.to_owned()),
    };
    let action = StatusAction {
        command: "plugins.install".to_owned(),
        label_key: NEW_ACTION_KEY.to_owned(),
        label_args: Default::default(),
        args: serde_json::json!({ "bundled": ids }),
    };
    core.notify_with_action(notice, action);
}

#[cfg(test)]
#[path = "bundle_tests.rs"]
mod tests;
