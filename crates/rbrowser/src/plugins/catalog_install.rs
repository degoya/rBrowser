//! Reviewing, installing and removing a catalogue offer (RB-091-01), split from `catalog.rs`.
//!
//! Two steps, both from the settings window only (`settings_window::guard`, security review L3):
//! `catalogPrepare` downloads and reviews the package and answers what the dialog shows - the
//! package's **full grants** as the trust dialog shows a project plugin's (CDP domains, globs,
//! sidecar methods, the window image, the risky ones named; review M1) and its publisher key:
//! trusted or not, and whether it is the installed version's (H1). `catalogInstall` downloads the
//! same indexed file again, reviews it again and installs it only when the grants and the
//! capabilities are exactly the ones shown and - for a key not trusted yet or changed - the key's
//! fingerprint was confirmed in its own step. A confirmed new key is pinned in
//! `plugins.trustedKeys`, as the settings window would pin it. RB-091-04 lifted the lock on
//! bundled plugins: the catalogue updates them too, from a package signed by the bundle's key -
//! one with a bundled plugin's id under another key is refused ([`refuse_foreign`]) - and an
//! install of a bundled plugin chooses it, so its copy stands in for the bundled package. An
//! install, update or removal takes effect live (RB-092-06, [`super::super::live::reload`]); the
//! answer says `restart: true` only when no load can run.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rb_config::{TrustedPluginKey, UserSettings};
use rb_ipc::{CoreToShell, IpcErrorCode, PluginGrantDetails};
use rb_plugin_host::{
    CatalogStore, Consent, HttpFetcher, IndexEntry, PinnedKey, Prepared, RepositoryError, Shipped,
};
use rb_settings::Scope;
use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;

use super::{open_catalog, store_dir};
use crate::control::Core;
use crate::ipc::Failure;

/// The failure code of a package with a bundled plugin's id signed by another key (RB-091-04).
pub const FOREIGN_CODE: &str = "plugins.catalog.error.foreign_bundled";

/// An install the catalogue page asks for.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallRequest {
    pub repository: String,
    pub id: String,
    pub version: String,
    pub consent: ConsentRequest,
}

/// What the install dialog showed and the person confirmed.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConsentRequest {
    pub capabilities: Vec<String>,
    /// The full grants the dialog showed (`catalogPrepare`'s `grants`).
    pub grants: PluginGrantDetails,
    /// Sent only after the key's own confirmation step.
    #[serde(default)]
    pub key_fingerprint: Option<String>,
}

/// What `catalogPrepare` answers: the review the dialog shows. Blocking.
///
/// # Errors
///
/// The failure's code.
pub fn review(core: &Core, repository: &str, id: &str, version: &str) -> Result<Value, Failure> {
    let user = core.user_settings();
    let (prepared, _, _, _) = fetch_and_prepare(&user, repository, id, version)?;
    refuse_foreign(id, &prepared.key().fingerprint, &bundled_keys())?;
    let capabilities: Vec<&str> = prepared.capability_ids().into_iter().collect();
    Ok(json!({ "value": { "review": {
        "repository": repository,
        "id": id,
        "version": version,
        "capabilities": capabilities,
        "grants": grants_of(&prepared),
        "key": prepared.key(),
    } } }))
}

/// Downloads, reviews and installs what `request` names when its consent covers the package;
/// pins a key confirmed in its own step. Blocking.
///
/// # Errors
///
/// The failure's code (`plugins.catalog.error.*` or the verifier's `plugins.error.*`).
pub fn install(core: &Core, request: InstallRequest) -> Result<Value, Failure> {
    let user = core.user_settings();
    let (prepared, bytes, entry, store) =
        fetch_and_prepare(&user, &request.repository, &request.id, &request.version)?;
    let bundled = bundled_keys();
    refuse_foreign(&request.id, &prepared.key().fingerprint, &bundled)?;
    // The full grants, not only the capability ids (security review M1).
    if !same_grants(&grants_of(&prepared), &request.consent.grants) {
        return Err(refusal(RepositoryError::ConsentMismatch));
    }
    let consent = Consent {
        capabilities: request.consent.capabilities,
        key_fingerprint: request.consent.key_fingerprint,
    };
    let (_, user_dir) = folders()?;
    let installed = rb_plugin_host::install_package(
        &prepared,
        &bytes,
        &request.repository,
        &entry,
        &consent,
        &user_dir,
        &store,
    )
    .map_err(refusal)?;
    tracing::info!(
        plugin_id = installed.id.as_str(),
        version = installed.version.as_str(),
        repository = request.repository.as_str(),
        "a plugin was installed from the catalogue"
    );
    if let Some(pin) = installed.pin {
        pin_key(core, &user, pin);
    }
    // The base of the next update check, also when it could not load live (review 6).
    crate::plugins::bundle_install::note_installed(&installed.id, &installed.version);
    // A bundled plugin installed from the catalogue is chosen: its copy stands in for the
    // bundled package (RB-091-04).
    if bundled.contains_key(&installed.id)
        && let Err(code) = super::super::bundle::update_choice(|state| state.choose(&installed.id))
    {
        tracing::warn!(
            code = code.as_str(),
            "the plugin was not added to the choice"
        );
    }
    let live = super::super::live::reload(core);
    Ok(json!({ "value": { "id": installed.id, "version": installed.version, "restart": !live } }))
}

/// The offer, downloaded and reviewed under `user`'s trusted keys.
fn fetch_and_prepare(
    user: &UserSettings,
    repository: &str,
    id: &str,
    version: &str,
) -> Result<(Prepared, Vec<u8>, IndexEntry, CatalogStore), Failure> {
    let (dir, _) = folders()?;
    let (catalog, _) = open_catalog(user, dir);
    let (url, entry) = catalog
        .offer(repository, id, version, OffsetDateTime::now_utc())
        .map_err(refusal)?;
    let bytes =
        rb_plugin_host::download_package(&HttpFetcher::new(), &url, &entry).map_err(refusal)?;
    let verifier = crate::analyzer::plugins::verifier(user);
    let store = catalog.store().clone();
    let prepared =
        rb_plugin_host::prepare_package(&bytes, &entry, &verifier, &store).map_err(refusal)?;
    Ok((prepared, bytes, entry, store))
}

/// The grants as the trust dialog shows them.
fn grants_of(prepared: &Prepared) -> PluginGrantDetails {
    crate::trust::grants::details(&prepared.manifest().grants)
}

/// Whether `shown` are exactly `actual`, whatever order the lists came in.
pub fn same_grants(actual: &PluginGrantDetails, shown: &PluginGrantDetails) -> bool {
    fn sorted(details: &PluginGrantDetails) -> PluginGrantDetails {
        let mut details = details.clone();
        for list in [
            &mut details.cdp_domains,
            &mut details.project_globs,
            &mut details.write_globs,
            &mut details.sidecar_methods,
            &mut details.risky,
        ] {
            list.sort();
        }
        details
    }
    sorted(actual) == sorted(shown)
}

/// Removes plugin `id` the catalogue installed; it leaves live (RB-092-06). Blocking.
///
/// # Errors
///
/// The failure's code.
pub fn remove(core: &Core, id: &str) -> Result<Value, Failure> {
    let (dir, user_dir) = folders()?;
    rb_plugin_host::remove_package(id, &user_dir, &CatalogStore::new(dir)).map_err(refusal)?;
    let live = super::super::live::reload(core);
    Ok(json!({ "value": { "id": id, "restart": !live } }))
}

/// Refuses a package of `id` signed by the key of `fingerprint` when `bundled` ships `id` under
/// another key: it could never stand in for the bundled plugin, and a repository may not take a
/// first-party plugin's id (RB-091-04).
///
/// # Errors
///
/// [`FOREIGN_CODE`].
pub fn refuse_foreign(
    id: &str,
    fingerprint: &str,
    bundled: &BTreeMap<String, String>,
) -> Result<(), Failure> {
    if bundled
        .get(id)
        .is_some_and(|shipped| shipped != fingerprint)
    {
        tracing::info!(
            plugin_id = id,
            "a package with a bundled plugin's id under another key is not installed"
        );
        return Err((IpcErrorCode::BadRequest, FOREIGN_CODE.to_owned()));
    }
    Ok(())
}

/// The key fingerprint of every bundled plugin, chosen or not, by id; waits for the load (a
/// blocking thread only).
fn bundled_keys() -> BTreeMap<String, String> {
    crate::plugins::bundle_offer::bundled_plugins()
        .into_iter()
        .filter_map(|(id, plugin)| {
            Shipped::of(&plugin.manifest).map(|shipped| (id, shipped.fingerprint))
        })
        .collect()
}

fn folders() -> Result<(PathBuf, PathBuf), Failure> {
    match (store_dir(), crate::analyzer::plugins::user_dir()) {
        (Some(dir), Some(user_dir)) => Ok((dir, user_dir)),
        _ => Err((
            IpcErrorCode::Internal,
            "plugins.catalog.error.io".to_owned(),
        )),
    }
}

/// The refusal as its code; only the code is logged (security review L6): a reason may carry an
/// address or a path.
fn refusal(error: RepositoryError) -> Failure {
    tracing::warn!(code = error.code(), "a plugin catalogue step was refused");
    (IpcErrorCode::BadRequest, error.code().to_owned())
}

/// `user`'s trusted keys with `pin` added once.
pub fn with_pinned(user: &UserSettings, pin: PinnedKey) -> Vec<TrustedPluginKey> {
    let mut keys = user.plugins.trusted_keys.clone();
    if !keys
        .iter()
        .any(|key| key.key_id == pin.key_id && key.fingerprint == pin.fingerprint)
    {
        keys.push(TrustedPluginKey {
            key_id: pin.key_id,
            public_key: pin.public_key,
            fingerprint: pin.fingerprint,
        });
    }
    keys
}

/// Pins `pin` in `plugins.trustedKeys` as the settings window writes it: a person confirmed it in
/// the key's own step.
fn pin_key(core: &Core, user: &UserSettings, pin: PinnedKey) {
    let Ok(value) = serde_json::to_value(with_pinned(user, pin)) else {
        return;
    };
    let answer = crate::settings_window::set_setting(
        core,
        "plugins.trustedKeys",
        &value,
        Scope::User,
        crate::settings_window::Caller::Shell,
    );
    if !matches!(answer, CoreToShell::Ack) {
        tracing::warn!("the confirmed plugin key could not be pinned in plugins.trustedKeys");
    }
}
