//! Installing and removing chosen first-party plugins (RB-091-04, adapted from rDownloader's
//! `plugin_bundled.rs`, RD-160-05, RD-1260-02, RD-180-14): the core side of the first-run wizard
//! (RB-091-05) and of the catalogue's bundled rows.
//!
//! - `pluginPanel { op: "available" }` ([`super::bundle_offer::available`]): every first-party plugin the bundle
//!   carries or the official repository offers, with name, description, group, size,
//!   recommendation, origin, capabilities and whether it is installed.
//! - `pluginPanel { op: "installChosen", ids }` ([`install_chosen`]): each from the repository
//!   when it offers a newer version than the bundle's, of the same key asking for nothing more
//!   and its host answers ([`rb_plugin_host::replacement`]), from the bundle otherwise - without a
//!   network everything comes from the bundle, and a host that failed rests for five minutes.
//!   One plugin that fails does not stop the others; a `pluginInstallProgress` message goes out
//!   before and after each. A plugin the package does not carry comes from the official
//!   repository, signed by a key this installation trusts.
//! - `pluginPanel { op: "removeChosen", ids }` ([`remove_chosen`]): takes them out of the choice
//!   and removes the copy the catalogue installed; no later start brings them back.
//! - `pluginPanel { op: "applyRecommended" }` ([`apply_recommended`], review 7): the wizard's
//!   Skip - the recommended set is chosen and the choice no longer waits for the wizard.
//!
//! The settings window's pages alone send these (`settings_window::guard`); MCP never does.
//! Everything loads at the next start, like every install. `plugins.catalog.preferRepository`
//! off leaves every install to the bundle; `plugins.catalog.autoUpdate` lets the update check
//! install what [`rb_plugin_host::newer_than`] allows ([`super::bundle_offer::auto_update`]).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use chrono::Utc;
use rb_config::UserSettings;
use rb_ipc::{CoreToShell, IpcErrorCode, PluginGrantDetails};
use rb_plugin_host::{
    Catalog, CatalogStore, Consent, HttpFetcher, OFFICIAL_REPOSITORY, Offer, Reachability, Shipped,
    replacement, valid_plugin_id,
};
use serde_json::{Value, json};

use crate::control::Core;
use crate::ipc::Failure;

/// The most plugins one request names.
const MAX_IDS: usize = 256;
/// A request without ids, with too many, or with one that is no plugin id.
pub const INVALID_CODE: &str = "plugins.bundle.error.invalid";
/// Neither the bundle nor the official repository has the plugin.
pub const UNKNOWN_CODE: &str = "plugins.bundle.error.unknown";
/// The plugin is only in the repository, and the repository cannot be reached.
pub const OFFLINE_CODE: &str = "plugins.bundle.error.offline";
/// The message key of the progress events.
pub const PROGRESS_KEY: &str = "pluginInstallProgress";

/// The hosts that failed lately, for the life of the process.
static REACHABILITY: LazyLock<Reachability> = LazyLock::new(Reachability::default);
/// The versions installed in this session, by plugin id: they load at the next start, and until
/// then they are the base of an update check (review 6).
static SESSION: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

/// Records that `version` of plugin `id` was installed in this session.
pub fn note_installed(id: &str, version: &str) {
    rb_util::lock(&SESSION).insert(id.to_owned(), version.to_owned());
}

/// `shipped` raised to the version of it installed in this session, when there is one.
pub fn with_session(shipped: Shipped) -> Shipped {
    match rb_util::lock(&SESSION).get(&shipped.id) {
        Some(version) => shipped.at_least(version),
        None => shipped,
    }
}

/// Whether `new` asks for nothing `old` does not grant.
pub fn grants_within(new: &PluginGrantDetails, old: &PluginGrantDetails) -> bool {
    fn subset(new: &[String], old: &[String]) -> bool {
        new.iter().all(|item| old.contains(item))
    }
    (!new.evaluate || old.evaluate)
        && (!new.window_image || old.window_image)
        && subset(&new.cdp_domains, &old.cdp_domains)
        && subset(&new.project_globs, &old.project_globs)
        && subset(&new.write_globs, &old.write_globs)
        && subset(&new.sidecar_methods, &old.sidecar_methods)
        && subset(&new.risky, &old.risky)
        && match (new.http_rate_per_second, old.http_rate_per_second) {
            (None, _) => true,
            (Some(new), Some(old)) => new <= old,
            (Some(_), None) => false,
        }
}

/// `ids` checked - between one and [`MAX_IDS`] plugin ids - each once, in order.
///
/// # Errors
///
/// [`INVALID_CODE`].
pub fn valid_ids(ids: Vec<String>) -> Result<Vec<String>, Failure> {
    let mut seen = BTreeSet::new();
    let ids: Vec<String> = ids
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect();
    if ids.is_empty() || ids.len() > MAX_IDS || !ids.iter().all(|id| valid_plugin_id(id)) {
        return Err((IpcErrorCode::BadRequest, INVALID_CODE.to_owned()));
    }
    Ok(ids)
}

/// The progress event of plugin `id`: `done` of `total` finished, its `state`.
pub fn progress_message(
    id: &str,
    done: usize,
    total: usize,
    state: &str,
    error: Option<&str>,
) -> Value {
    let mut progress = json!({ "id": id, "done": done, "total": total, "state": state });
    if let (Some(error), Value::Object(fields)) = (error, &mut progress) {
        fields.insert("error".to_owned(), Value::from(error));
    }
    let mut message = serde_json::Map::new();
    message.insert(PROGRESS_KEY.to_owned(), progress);
    Value::Object(message)
}

fn progress(core: &Core, id: &str, done: usize, total: usize, state: &str, error: Option<&str>) {
    core.send(&CoreToShell::PluginPanel {
        message: progress_message(id, done, total, state, error),
    });
}

/// The catalogue of `user`'s repositories; with `online`, the official index fetched first when
/// none is loaded and its host did not just fail. The flag says the official index is missing.
pub(super) fn open(user: &UserSettings, online: bool) -> Option<(Catalog, bool)> {
    let dir = super::catalog::store_dir()?;
    let (mut catalog, _) = super::catalog::open_catalog(user, dir);
    let now = Utc::now();
    let official = catalog
        .repositories()
        .iter()
        .find(|repository| repository.is_official())
        .map(|repository| repository.url.clone());
    if online
        && let Some(url) = official
        && !catalog.is_loaded(OFFICIAL_REPOSITORY, now)
        && REACHABILITY.reachable(&url, Instant::now())
    {
        let outcomes = catalog.refresh(&HttpFetcher::new(), now);
        let failed = outcomes
            .iter()
            .any(|(id, outcome)| id == OFFICIAL_REPOSITORY && outcome.is_err());
        REACHABILITY.note(&url, !failed, Instant::now());
    }
    let missing = !catalog.is_loaded(OFFICIAL_REPOSITORY, Utc::now());
    Some((catalog, missing))
}

/// Every repository's offer of plugin `id`.
pub(super) fn offers_of(catalog: &Catalog, id: &str) -> Vec<Offer> {
    catalog
        .entries(Utc::now())
        .into_iter()
        .find(|entry| entry.id == id)
        .map(|entry| entry.offers)
        .unwrap_or_default()
}

/// Downloads, reviews and installs `offer` into the person's plugin folder, under a key this
/// installation trusts and - with `within` - with no grant beyond it.
pub(super) fn install_offer(
    user: &UserSettings,
    catalog: &Catalog,
    offer: &Offer,
    within: Option<&PluginGrantDetails>,
) -> Result<(), String> {
    let code = |error: rb_plugin_host::RepositoryError| error.code().to_owned();
    let (url, entry) = catalog
        .offer(
            &offer.repository,
            &offer.entry.id,
            &offer.entry.version,
            Utc::now(),
        )
        .map_err(code)?;
    if !REACHABILITY.reachable(&url, Instant::now()) {
        return Err(OFFLINE_CODE.to_owned());
    }
    let bytes = match rb_plugin_host::download_package(&HttpFetcher::new(), &url, &entry) {
        Ok(bytes) => {
            REACHABILITY.note(&url, true, Instant::now());
            bytes
        }
        Err(error) => {
            REACHABILITY.note(&url, false, Instant::now());
            return Err(code(error));
        }
    };
    let verifier = crate::analyzer::plugins::verifier(user);
    let store = catalog.store().clone();
    let prepared =
        rb_plugin_host::prepare_package(&bytes, &entry, &verifier, &store).map_err(code)?;
    if !prepared.key().trusted || prepared.key().changed() {
        return Err("plugins.catalog.error.key_untrusted".to_owned());
    }
    let granted = crate::trust::grants::details(&prepared.manifest().grants);
    if within.is_some_and(|within| !grants_within(&granted, within)) {
        return Err("plugins.catalog.error.consent_mismatch".to_owned());
    }
    let consent = Consent {
        capabilities: prepared
            .capability_ids()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        key_fingerprint: Some(prepared.key().fingerprint.clone()),
    };
    let user_dir = crate::analyzer::plugins::user_dir()
        .ok_or_else(|| "plugins.catalog.error.io".to_owned())?;
    let installed = rb_plugin_host::install_package(
        &prepared,
        &bytes,
        &offer.repository,
        &entry,
        &consent,
        &user_dir,
        &store,
    )
    .map_err(code)?;
    note_installed(&installed.id, &installed.version);
    tracing::info!(
        plugin_id = installed.id.as_str(),
        version = installed.version.as_str(),
        repository = offer.repository.as_str(),
        "a first-party plugin was installed from the repository; it loads at the next start"
    );
    Ok(())
}

/// Installs one chosen plugin; its origin, or the origin it tried and the failure's code.
fn install_one(
    user: &UserSettings,
    bundle: &BTreeMap<String, super::bundle_offer::BundledPlugin>,
    installed: &BTreeSet<String>,
    opened: Option<&(Catalog, bool)>,
    id: &str,
) -> Result<&'static str, (&'static str, String)> {
    if let Some(plugin) = bundle.get(id) {
        let mut origin = "bundle";
        let candidate = opened
            .filter(|_| user.plugins.catalog.prefer_repository && !plugin.installed)
            .zip(Shipped::of(&plugin.manifest));
        let offers = candidate
            .as_ref()
            .map(|((catalog, _), _)| offers_of(catalog, id))
            .unwrap_or_default();
        if let Some(((catalog, _), shipped)) = &candidate
            && let Some(offer) = replacement(&offers, shipped)
        {
            let within = crate::trust::grants::details(&plugin.manifest.grants);
            match install_offer(user, catalog, offer, Some(&within)) {
                Ok(()) => origin = "repository",
                Err(code) => tracing::info!(
                    plugin_id = id,
                    code = code.as_str(),
                    "the repository's version did not install; the bundled one is chosen"
                ),
            }
        }
        super::bundle::update_choice(|state| state.choose(id)).map_err(|code| (origin, code))?;
        return Ok(origin);
    }
    if installed.contains(id) {
        return Ok("repository");
    }
    if !super::first_party::is_first_party(id) {
        return Err(("repository", UNKNOWN_CODE.to_owned()));
    }
    let Some((catalog, missing)) = opened else {
        return Err(("repository", OFFLINE_CODE.to_owned()));
    };
    let offers = offers_of(catalog, id);
    let Some(offer) = offers.iter().find(|offer| {
        offer.repository == OFFICIAL_REPOSITORY && offer.compatibility.is_compatible()
    }) else {
        let code = if *missing { OFFLINE_CODE } else { UNKNOWN_CODE };
        return Err(("repository", code.to_owned()));
    };
    install_offer(user, catalog, offer, None).map_err(|code| ("repository", code))?;
    Ok("repository")
}

/// The answer to `installChosen`, after every plugin. Blocking.
///
/// # Errors
///
/// [`INVALID_CODE`] for the request as a whole; a plugin's failure is its result's.
pub fn install_chosen(core: &Core, ids: Vec<String>) -> Result<Value, Failure> {
    let ids = valid_ids(ids)?;
    let user = core.user_settings();
    let bundle = super::bundle_offer::bundled_plugins();
    let installed = super::bundle::installed_ids().unwrap_or_default();
    let online =
        user.plugins.catalog.prefer_repository || ids.iter().any(|id| !bundle.contains_key(id));
    let opened = open(&user, online);
    let total = ids.len();
    let mut results = Vec::new();
    for (done, id) in ids.iter().enumerate() {
        progress(core, id, done, total, "installing", None);
        match install_one(&user, &bundle, &installed, opened.as_ref(), id) {
            Ok(origin) => {
                progress(core, id, done + 1, total, "installed", None);
                results.push(json!({ "id": id, "ok": true, "origin": origin }));
            }
            Err((origin, code)) => {
                progress(core, id, done + 1, total, "failed", Some(&code));
                results.push(json!({ "id": id, "ok": false, "error": code, "origin": origin }));
            }
        }
    }
    Ok(json!({ "value": { "results": results, "restart": true } }))
}

/// The answer to `removeChosen`. Blocking.
///
/// # Errors
///
/// [`INVALID_CODE`] for the request as a whole; a plugin's failure is its result's.
pub fn remove_chosen(ids: Vec<String>) -> Result<Value, Failure> {
    let ids = valid_ids(ids)?;
    let folders = super::catalog::store_dir().zip(crate::analyzer::plugins::user_dir());
    let mut results = Vec::new();
    for id in &ids {
        let mut outcome = super::bundle::update_choice(|state| state.unchoose(id)).map(drop);
        if outcome.is_ok()
            && let Some((dir, user_dir)) = &folders
        {
            let store = CatalogStore::new(dir.clone());
            if store.installs().contains_key(id) {
                outcome = rb_plugin_host::remove_package(id, user_dir, &store)
                    .map_err(|error| error.code().to_owned());
            }
        }
        results.push(match outcome {
            Ok(()) => json!({ "id": id, "ok": true }),
            Err(code) => json!({ "id": id, "ok": false, "error": code }),
        });
    }
    Ok(json!({ "value": { "results": results, "restart": true } }))
}

/// The answer to `applyRecommended`, the first-run wizard's Skip (review 7): the recommended part
/// of the bundle is added to the choice, which no longer waits for the wizard. Loads at the next
/// start. Blocking.
///
/// # Errors
///
/// The code of a choice that was not saved.
pub fn apply_recommended() -> Result<Value, Failure> {
    let bundled: BTreeSet<String> = super::bundle_offer::bundled_plugins().into_keys().collect();
    let recommended = super::first_party::recommended();
    let state =
        super::bundle::update_choice(|state| state.apply_recommended(&bundled, &recommended))
            .map_err(|code| (IpcErrorCode::Internal, code))?;
    Ok(json!({ "value": { "chosen": state.chosen, "restart": true } }))
}

#[cfg(test)]
#[path = "bundle_install_tests.rs"]
mod tests;
