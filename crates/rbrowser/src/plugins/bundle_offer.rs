//! What the first-run wizard and the catalogue list of the first-party plugins (RB-091-04),
//! split from `bundle_install.rs`: the bundle of this start ([`bundled_plugins`]), the wizard's
//! `available` answer ([`available`]), the catalogue's bundled rows with their updates
//! ([`bundled_rows`]) and the updates `plugins.catalog.autoUpdate` installs by itself
//! ([`auto_update`]).

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::Utc;
use rb_config::UserSettings;
use rb_plugin_host::{
    OFFICIAL_REPOSITORY, Offer, PluginManifest, PluginSource, Shipped, newer_than, replacement,
};
use serde_json::{Value, json};

use super::bundle_install::{install_offer, offers_of, open};
use crate::control::Core;

/// A first-party plugin the bundle carries.
pub(crate) struct BundledPlugin {
    pub(crate) manifest: PluginManifest,
    pub(crate) path: PathBuf,
    /// Chosen and loaded (or switched off) at this start.
    pub(crate) installed: bool,
}

/// The bundle of this start, by id. Waits for the load: a blocking thread only.
pub(crate) fn bundled_plugins() -> BTreeMap<String, BundledPlugin> {
    let report = &crate::analyzer::plugins::bundled().report;
    let mut found = BTreeMap::new();
    let mut add = |manifest: &PluginManifest, path: &PathBuf, installed: bool| {
        found
            .entry(manifest.id.clone())
            .or_insert_with(|| BundledPlugin {
                manifest: manifest.clone(),
                path: path.clone(),
                installed,
            });
    };
    for entry in report
        .loaded
        .iter()
        .filter(|entry| entry.source == PluginSource::Bundled)
    {
        add(entry.plugin.manifest(), &entry.path, true);
    }
    for entry in report
        .disabled
        .iter()
        .filter(|entry| entry.source == PluginSource::Bundled)
    {
        add(&entry.manifest, &entry.path, true);
    }
    for entry in &report.available {
        add(&entry.manifest, &entry.path, false);
    }
    found
}

/// One plugin as the wizard lists it.
#[derive(Clone, Copy, Debug)]
pub struct Row<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub version: &'a str,
    pub size: u64,
    pub capabilities: &'a [String],
    pub installed: bool,
    /// `bundle` or `repository`: where an install takes it from.
    pub origin: &'a str,
}

/// `row` as the wizard reads it (RB-091-05).
pub fn row(row: Row<'_>) -> Value {
    json!({
        "id": row.id,
        "name": row.name,
        "description": row.description,
        "category": super::first_party::category(row.id).unwrap_or("tools"),
        "sizeBytes": row.size,
        "recommended": super::first_party::recommended().contains(row.id),
        "origin": row.origin,
        "grants": row.capabilities,
        "installed": row.installed,
        "version": row.version,
    })
}

fn capability_ids(manifest: &PluginManifest) -> Vec<String> {
    manifest
        .grants
        .granted
        .iter()
        .map(|capability| capability.id().to_owned())
        .collect()
}

/// The answer to `available`. Blocking.
pub fn available(core: &Core) -> Value {
    let user = core.user_settings();
    let bundle = bundled_plugins();
    let installed = super::bundle::installed_ids().unwrap_or_default();
    let prefer = user.plugins.catalog.prefer_repository;
    let opened = open(&user, prefer);
    let entries = opened
        .as_ref()
        .map(|(catalog, _)| catalog.entries(Utc::now()))
        .unwrap_or_default();
    let offers = |id: &str| -> Vec<Offer> {
        entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.offers.clone())
            .unwrap_or_default()
    };
    let mut plugins = Vec::new();
    for (id, plugin) in &bundle {
        let from_repository = prefer
            && Shipped::of(&plugin.manifest)
                .is_some_and(|shipped| replacement(&offers(id), &shipped).is_some());
        let version = plugin.manifest.version.to_string();
        let capabilities = capability_ids(&plugin.manifest);
        plugins.push(row(Row {
            id,
            name: &plugin.manifest.name,
            description: &plugin.manifest.description,
            version: &version,
            size: std::fs::metadata(&plugin.path).map_or(0, |meta| meta.len()),
            capabilities: &capabilities,
            installed: plugin.installed,
            origin: if from_repository {
                "repository"
            } else {
                "bundle"
            },
        }));
    }
    // What only the official repository offers of the first-party plugins.
    for entry in entries.iter().filter(|entry| {
        !bundle.contains_key(&entry.id) && super::first_party::is_first_party(&entry.id)
    }) {
        let Some(offer) = entry.offers.iter().find(|offer| {
            offer.repository == OFFICIAL_REPOSITORY && offer.compatibility.is_compatible()
        }) else {
            continue;
        };
        plugins.push(row(Row {
            id: &entry.id,
            name: &offer.entry.name,
            description: &offer.entry.description,
            version: &offer.entry.version,
            size: offer.entry.size,
            capabilities: &offer.entry.capabilities,
            installed: installed.contains(&entry.id),
            origin: "repository",
        }));
    }
    let offline = opened.as_ref().is_none_or(|(_, missing)| *missing);
    // `choicePending`: a fresh installation's choice waits for the wizard (review 7); nothing
    // optional is installed yet, so every row says `installed: false` until it answers.
    json!({ "value": {
        "offline": offline,
        "choicePending": super::bundle::choice_pending(),
        "plugins": plugins,
    } })
}

/// The bundled plugins for the catalogue page: each with its origin and the update a
/// repository offers under the rule of [`newer_than`]. Reads the cached indexes only. Blocking.
pub fn bundled_rows(user: &UserSettings) -> Vec<Value> {
    let opened = open(user, false);
    let offers = |id: &str| {
        opened
            .as_ref()
            .map(|(catalog, _)| offers_of(catalog, id))
            .unwrap_or_default()
    };
    bundled_plugins()
        .iter()
        .map(|(id, plugin)| {
            // What this session installed already is the base (review 6).
            let update = Shipped::of(&plugin.manifest)
                .map(super::bundle_install::with_session)
                .and_then(|shipped| {
                    newer_than(&offers(id), &shipped)
                        .map(|offer| (offer.repository.clone(), offer.entry.version.clone()))
                })
                .filter(|_| plugin.installed)
                .map(
                    |(repository, version)| json!({ "repository": repository, "version": version }),
                );
            let version = plugin.manifest.version.to_string();
            let capabilities = capability_ids(&plugin.manifest);
            let mut value = row(Row {
                id,
                name: &plugin.manifest.name,
                description: &plugin.manifest.description,
                version: &version,
                size: std::fs::metadata(&plugin.path).map_or(0, |meta| meta.len()),
                capabilities: &capabilities,
                installed: plugin.installed,
                origin: "bundle",
            });
            if let Value::Object(fields) = &mut value {
                fields.insert("update".to_owned(), update.unwrap_or(Value::Null));
                // The catalogue's kind filter.
                let kinds: Vec<&str> = plugin
                    .manifest
                    .kinds
                    .iter()
                    .map(|kind| kind.world())
                    .collect();
                fields.insert("kinds".to_owned(), json!(kinds));
            }
            value
        })
        .collect()
}

/// Installs every update the repositories offer of a bundled plugin or one the catalogue
/// installed that is signed by the same key and asks for nothing more, when
/// `plugins.catalog.autoUpdate` is on; how many. Blocking.
pub fn auto_update(core: &Core) -> usize {
    let user = core.user_settings();
    if !user.plugins.catalog.auto_update {
        return 0;
    }
    let Some((catalog, _)) = open(&user, true) else {
        return 0;
    };
    let installs = catalog.store().installs();
    let report = &crate::analyzer::plugins::bundled().report;
    let mut count = 0;
    for entry in &report.loaded {
        let manifest = entry.plugin.manifest();
        // Never a project's plugin, never one copied in by hand.
        if entry.source != PluginSource::Bundled && !installs.contains_key(&manifest.id) {
            continue;
        }
        // An update this session installed already is not fetched again (review 6).
        let Some(shipped) = Shipped::of(manifest).map(super::bundle_install::with_session) else {
            continue;
        };
        let offers = offers_of(&catalog, &manifest.id);
        let Some(offer) = newer_than(&offers, &shipped) else {
            continue;
        };
        let within = crate::trust::grants::details(&manifest.grants);
        match install_offer(&user, &catalog, offer, Some(&within)) {
            Ok(()) => count += 1,
            Err(code) => tracing::info!(
                plugin_id = manifest.id.as_str(),
                code = code.as_str(),
                "an update was not installed by itself; the catalogue offers it"
            ),
        }
    }
    count
}

#[cfg(test)]
#[path = "bundle_offer_tests.rs"]
mod tests;
