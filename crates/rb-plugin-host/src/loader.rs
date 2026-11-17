//! Where plugins come from and how a start loads them (`docs/plugins.md`, "Packages, signing and
//! trust"): the bundled first-party plugins, the ones a person installed, and the project's own
//! in `.rbrowser/plugins/` - **only under workspace trust**.
//!
//! New in rBrowser (rCommander loads from its installation folder alone). The order decides a
//! collision: bundled first, then user-installed, then project plugins, so a project can never
//! shadow a plugin the person relies on. Every package is verified again at every start; a
//! refusal is reported with its code and never stops the others.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use rb_plugin_types::{Capability, Failure, FailureKind, PluginKind, codes, paths};

use crate::{
    commands::plugin_command_defs,
    health::PluginHealths,
    manifest::PluginManifest,
    package::{MANIFEST_MEMBER, PACKAGE_EXTENSION, VerifiedPackage},
    plugin::{LoadedPlugin, PluginOptions},
    refusal::refusal_failure,
    registry::Registry,
    sinks::{LocaleSink, SettingsSink, WorkspaceTrust},
    verifier::{PluginVerifier, VerifyError},
};

/// The most packages read from one folder.
const MAX_PACKAGES_PER_FOLDER: usize = 256;

/// Where a plugin came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginSource {
    /// Shipped beside `rbrowser.exe`, signed with rBrowser's key.
    Bundled,
    /// Installed by the person in Settings > Plugins.
    User,
    /// Checked in with the project, `.rbrowser/plugins/`.
    Project,
}

/// The folders a start reads.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    pub bundled: Option<PathBuf>,
    pub user: Option<PathBuf>,
    pub project_root: Option<PathBuf>,
}

/// `.rbrowser/plugins/` of the project at `root`.
#[must_use]
pub fn project_plugins_dir(root: &Path) -> PathBuf {
    root.join(paths::CONFIG_DIR).join("plugins")
}

/// One package that did not load, and why.
#[derive(Debug)]
pub struct Refused {
    pub source: PluginSource,
    pub path: PathBuf,
    pub failure: Failure,
    /// What the package's manifest says it may do, read without trusting it - for the trust
    /// prompt, which lists a project plugin's capabilities.
    pub capabilities: Vec<Capability>,
    /// `(key_id, fingerprint)` of a key a person may confirm (trust on first use).
    pub untrusted_key: Option<(String, String)>,
    /// Whether the package carries a signature, once it verified (`false`: unsigned, accepted
    /// in development mode); `None` when the verification itself refused it (RB-080-51).
    pub signed: Option<bool>,
    /// What the package's manifest says of itself, read once at the load **without verifying
    /// it**: text to show a person as unverified, never an identity and never for an agent.
    /// `None` when it does not read - and for a project package of an untrusted workspace,
    /// whose text is not taken at all (RB-080-51).
    pub claims: Option<Box<PluginManifest>>,
}

/// One plugin loaded and activated.
pub struct LoadedEntry {
    pub source: PluginSource,
    pub path: PathBuf,
    pub plugin: LoadedPlugin,
    /// Notices of the load, such as a default shortcut left unbound.
    pub notices: Vec<Failure>,
    /// Its `page/` and `panel/` members, for the page scripts and panels (RB-060-12).
    pub ui_files: Arc<BTreeMap<String, Vec<u8>>>,
    /// Whether the package is signed (`false`: unsigned, accepted in development mode).
    pub signed: bool,
}

/// One verified package the person switched off (`plugins.disabled`): not loaded, but listed in
/// Settings > Plugins (RB-080-51).
#[derive(Clone, Debug)]
pub struct DisabledEntry {
    pub source: PluginSource,
    pub path: PathBuf,
    pub manifest: PluginManifest,
    /// As [`LoadedEntry::signed`].
    pub signed: bool,
}

/// What a start loaded and refused.
#[derive(Default)]
pub struct LoadReport {
    pub loaded: Vec<LoadedEntry>,
    pub refused: Vec<Refused>,
    /// The packages skipped for `plugins.disabled`.
    pub disabled: Vec<DisabledEntry>,
}

/// What a load needs besides the folders.
pub struct LoadContext<'a> {
    pub verifier: &'a PluginVerifier,
    pub trust: &'a dyn WorkspaceTrust,
    pub locales: &'a dyn LocaleSink,
    pub settings: &'a dyn SettingsSink,
    /// The services and the project the plugins share; `settings` is replaced per plugin by
    /// [`SettingsSink::values`] and `health` by the plugin's own from [`LoadContext::healths`].
    pub options: PluginOptions,
    /// Every plugin's own health, by id (RB-090-59): never one shared by two plugins, whose
    /// failures, suspension and running calls would then be each other's.
    pub healths: &'a PluginHealths,
    /// The ids the person disabled (`plugins.disabled`); skipped, their settings kept.
    pub disabled: BTreeSet<String>,
}

/// The `.rbplugin` files of `sources`, in loading order.
#[must_use]
pub fn discover(sources: &Sources) -> Vec<(PluginSource, PathBuf)> {
    let project = sources.project_root.as_deref().map(project_plugins_dir);
    [
        (PluginSource::Bundled, sources.bundled.clone()),
        (PluginSource::User, sources.user.clone()),
        (PluginSource::Project, project),
    ]
    .into_iter()
    .filter_map(|(source, folder)| folder.map(|folder| (source, folder)))
    .flat_map(|(source, folder)| {
        packages_in(&folder)
            .into_iter()
            .map(move |path| (source, path))
    })
    .collect()
}

fn packages_in(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == PACKAGE_EXTENSION))
        .collect();
    found.sort();
    found.truncate(MAX_PACKAGES_PER_FOLDER);
    found
}

/// Verifies, admits, loads and activates every package of `sources`.
pub async fn load_all(
    sources: &Sources,
    context: &LoadContext<'_>,
    registry: &mut Registry,
) -> LoadReport {
    let mut report = LoadReport::default();
    let project_trusted = sources
        .project_root
        .as_deref()
        .is_some_and(|root| context.trust.is_trusted(root));
    for (source, path) in discover(sources) {
        if source == PluginSource::Project && !project_trusted {
            report.refused.push(Refused {
                source,
                capabilities: declared_capabilities(&path),
                path,
                failure: Failure::coded(FailureKind::AccessDenied, codes::UNTRUSTED_WORKSPACE),
                untrusted_key: None,
                signed: None,
                claims: None,
            });
            continue;
        }
        match load_one(&path, context, registry).await {
            Ok(Outcome::Loaded(loaded)) => {
                let (plugin, notices, ui_files, signed) = *loaded;
                report.loaded.push(LoadedEntry {
                    source,
                    path,
                    plugin,
                    notices,
                    ui_files: Arc::new(ui_files),
                    signed,
                });
            }
            Ok(Outcome::Disabled(manifest, signed)) => report.disabled.push(DisabledEntry {
                source,
                path,
                manifest: *manifest,
                signed,
            }),
            Err((failure, untrusted_key, signed)) => {
                let claims = declared_manifest(&path).map(Box::new);
                report.refused.push(Refused {
                    source,
                    capabilities: claims
                        .as_ref()
                        .map(|manifest| manifest.grants.granted.iter().copied().collect())
                        .unwrap_or_default(),
                    path,
                    failure,
                    untrusted_key,
                    signed,
                    claims,
                });
            }
        }
    }
    report
}

/// The failure, the key a person may confirm, and whether the package is signed when it
/// verified.
type LoadFailure = (Failure, Option<(String, String)>, Option<bool>);
type Loaded = (LoadedPlugin, Vec<Failure>, BTreeMap<String, Vec<u8>>, bool);

/// What became of one verified package.
enum Outcome {
    Loaded(Box<Loaded>),
    /// Skipped for `plugins.disabled`: its manifest and whether it is signed.
    Disabled(Box<PluginManifest>, bool),
}

async fn load_one(
    path: &Path,
    context: &LoadContext<'_>,
    registry: &mut Registry,
) -> Result<Outcome, LoadFailure> {
    let package = context
        .verifier
        .verify_file(path)
        .map_err(|error| match error {
            VerifyError::UntrustedKey {
                key_id,
                fingerprint,
                ..
            } => (
                Failure::coded(FailureKind::AccessDenied, codes::KEY_UNTRUSTED)
                    .with_param("fingerprint", &fingerprint),
                Some((key_id, fingerprint)),
                None,
            ),
            VerifyError::Other(error) => (refusal_failure(&error), None, None),
        })?;
    let signed = package.signature.is_some();
    if context.disabled.contains(&package.manifest.id) {
        return Ok(Outcome::Disabled(Box::new(package.manifest), signed));
    }
    registry
        .admit(&package.manifest)
        .map_err(|failure| (failure, None, Some(signed)))?;
    let loaded = activate(&package, context).await;
    if loaded.is_err() {
        registry.remove(&package.manifest.id);
        context.locales.unregister(&package.manifest.id);
    }
    loaded
        .map(|(plugin, notices)| {
            Outcome::Loaded(Box::new((plugin, notices, package.ui_files(), signed)))
        })
        .map_err(|failure| (failure, None, Some(signed)))
}

async fn activate(
    package: &VerifiedPackage,
    context: &LoadContext<'_>,
) -> Result<(LoadedPlugin, Vec<Failure>), Failure> {
    // Each plugin reads its own settings and has its own health, never one another's.
    let options = PluginOptions {
        settings: context.settings.values(&package.manifest.id),
        health: context.healths.of(&package.manifest.id),
        ..context.options.clone()
    };
    let plugin = LoadedPlugin::load(package, options)?;
    let id = &package.manifest.id;
    for (language, bytes) in package.locales() {
        context
            .locales
            .register(id, language, bytes)
            .map_err(|reason| {
                Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
            })?;
    }
    if let Some(schema) = package.settings_schema() {
        let schema = crate::contents::check_settings_schema(schema)
            .map_err(|error| refusal_failure(&error))?;
        context
            .settings
            .register_plugin_schema(id, &schema)
            .map_err(|reason| {
                Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
            })?;
    }
    if package.manifest.is(PluginKind::Feature) {
        plugin.activate().await?;
    }
    let (_, notices) = plugin_command_defs(&package.manifest);
    Ok((plugin, notices))
}

/// The capabilities a package's manifest declares, read without verifying it: shown (the trust
/// dialog lists a project plugin's), never granted. Empty when the package does not read.
#[must_use]
pub fn declared_capabilities(path: &Path) -> Vec<Capability> {
    declared_grants(path)
        .map(|grants| grants.granted.into_iter().collect())
        .unwrap_or_default()
}

/// The `[capabilities]` a package's manifest declares with their settings - CDP domains, globs,
/// sidecar methods, the HTTP rate - read without verifying it: shown in the trust dialog
/// (review finding M1 of Charge 6), never granted. `None` when the package does not read.
#[must_use]
pub fn declared_grants(path: &Path) -> Option<crate::manifest::Grants> {
    declared_manifest(path).map(|manifest| manifest.grants)
}

/// A package's manifest, read without verifying it ([`Refused::claims`], the declared grants):
/// shown, never trusted. `None` when the package does not read.
fn declared_manifest(path: &Path) -> Option<PluginManifest> {
    let read = || -> anyhow::Result<PluginManifest> {
        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let member = archive.by_name(MANIFEST_MEMBER)?;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(
            &mut std::io::Read::take(member, crate::manifest::MAX_MANIFEST_BYTES as u64 + 1),
            &mut bytes,
        )?;
        crate::manifest::parse_manifest(&bytes)
    };
    read().ok()
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
