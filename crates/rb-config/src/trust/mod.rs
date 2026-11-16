//! Workspace trust (RB-010-09, contract 0.1-w1b): which parts of a checked-in `.rbrowser/` may
//! run code or redirect traffic, and whether the person trusts this project in its current form.
//!
//! An untrusted project loads breakpoints, layout, environments and emulation without headers;
//! everything of [`ProtectedKind`] stays blocked until a human trusts it in the shell. A trust
//! decision covers the project path and the [`TrustState::hash`] of its protected parts: when
//! they change, the project asks again. Decisions live in [`TRUST_FILE`] beside the user
//! settings ([`trust_store_path`]), never in a settings key, so neither `settings_set` nor any
//! MCP tool can reach them.
//!
//! Every place that loads a protected part asks [`trust_state`] (one central check); the
//! project config itself loads through [`load_project_trusted`], which applies the trust-gated
//! network keys ([`crate::TRUST_GATED_KEYS`]) only to a trusted workspace. A caller treats an
//! error as "not trusted". What is protected and how the hash is built: [`hash`]; the store on
//! disk, changed under a file lock: [`store`].

mod hash;
mod store;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use hash::PROTECTED_DIRS;

use crate::{ConfigError, LoadedConfig};

/// The trust store's file name inside [`crate::roaming_dir`].
pub const TRUST_FILE: &str = "trust.json";
/// The format version of [`TrustStore`].
pub const TRUST_STORE_VERSION: u32 = 1;

/// A part of a project that needs trust.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProtectedKind {
    /// `.rbrowser/flows/` (RB-040-07).
    Flows,
    /// Snippets (RB-070-04).
    Snippets,
    /// Project plugins, `.rbrowser/plugins/` (RB-010-03).
    Plugins,
    /// `.rbrowser/mocks/` (RB-070-02).
    Mocks,
    /// Request headers of `config.json` (`emulation.headers`, `breakpoints[].headers`).
    Headers,
    /// `network.hostResolverRules`.
    HostResolverRules,
    /// Page scripts of the project.
    PageScripts,
    /// `extensions` recommendations (RB-080-06).
    Extensions,
    /// `network.ignoreCertificateErrors` of `config.local.json` (local-only in `config.json`).
    IgnoreCertificateErrors,
    /// `network.proxy` of `config.local.json` (local-only in `config.json`).
    Proxy,
}

/// One protected part the project has.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedItem {
    /// What it is.
    pub kind: ProtectedKind,
    /// Where: a path relative to the project root with `/` (`.rbrowser/flows/a.ts`, a top-level
    /// entry of a protected folder), a JSON pointer into `config.json`
    /// (`/breakpoints/2/headers`), or one into `config.local.json` after its path
    /// (`.rbrowser/config.local.json#/network/proxy`).
    pub path: String,
}

/// The trust of one project as loaded now.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustState {
    /// The person trusts the project with exactly these protected parts.
    pub trusted: bool,
    /// The content hash of the protected parts (lower-case hex SHA-256); empty when the project
    /// has none.
    pub hash: String,
    /// Every protected part of the project; blocked while `trusted` is `false`.
    pub blocked: Vec<BlockedItem>,
    /// Links in a protected folder (paths relative to the root, `/`) that lead out of the
    /// project, nowhere, or to neither a file nor a folder: no hash pins what they load, so
    /// while any is there the project is not trusted, whatever the store says.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unpinned: Vec<String>,
}

impl TrustState {
    /// Whether protected parts of `kind` may load: only in a trusted project. One decision
    /// covers every kind (the dialog lists them all).
    pub fn allows(&self, kind: ProtectedKind) -> bool {
        let _ = kind;
        self.trusted
    }
}

/// One decision of the trust store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustRecord {
    /// The project root as opened (`\\wsl.localhost\...` included).
    pub root: String,
    /// [`TrustState::hash`] at the decision.
    pub hash: String,
    /// When, RFC 3339 UTC.
    pub decided_at: String,
}

/// The trust store, [`TRUST_FILE`]: written under a file lock (several instances).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustStore {
    /// [`TRUST_STORE_VERSION`].
    pub version: u32,
    /// Every decision, one per root.
    pub records: Vec<TrustRecord>,
}

impl TrustStore {
    /// The decision for the project at `root`, if any.
    pub fn record(&self, root: &Path) -> Option<&TrustRecord> {
        let key = root_key(&root.to_string_lossy());
        self.records
            .iter()
            .find(|record| root_key(&record.root) == key)
    }

    /// Trusts the project at `root` with the protected parts of `hash` (the
    /// [`TrustState::hash`] the person was shown), replacing an earlier decision for it.
    /// Called only for the shell dialog's `trustDecision`.
    pub fn grant(&mut self, root: &Path, hash: &str, decided_at: &str) {
        self.revoke(root);
        self.records.push(TrustRecord {
            root: root.to_string_lossy().into_owned(),
            hash: hash.to_owned(),
            decided_at: decided_at.to_owned(),
        });
    }

    /// Withdraws the decision for `root`; `false` when there was none.
    pub fn revoke(&mut self, root: &Path) -> bool {
        let key = root_key(&root.to_string_lossy());
        let before = self.records.len();
        self.records.retain(|record| root_key(&record.root) != key);
        self.records.len() != before
    }
}

/// A root as compared: `/` for `\`, without a `\\?\` prefix or a trailing separator, and on
/// Windows without case.
fn root_key(root: &str) -> String {
    let mut key = root.replace('\\', "/");
    if let Some(rest) = key.strip_prefix("//?/UNC/") {
        key = format!("//{rest}");
    } else if let Some(rest) = key.strip_prefix("//?/") {
        key = rest.to_owned();
    }
    while key.len() > 1 && key.ends_with('/') {
        key.pop();
    }
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}

/// The trust store's path, `%APPDATA%\rBrowser\trust.json` (portable: beside the executable):
/// [`trust_store_for`] the user settings file of this machine, so the start-up, the plugin
/// loader and the shell (which knows only its settings file) read one store.
pub fn trust_store_path() -> Option<PathBuf> {
    crate::user_settings_path().map(|settings| trust_store_for(&settings))
}

/// The trust store that belongs to the user settings file at `settings`: [`TRUST_FILE`] beside
/// it. The one rule every reader of the store goes through.
pub fn trust_store_for(settings: &Path) -> PathBuf {
    settings.with_file_name(TRUST_FILE)
}

/// Loads the trust store at `path`; a missing file is an empty store. A broken file or a newer
/// `version` is an error, which the caller treats as "nothing trusted".
pub fn load_trust_store(path: &Path) -> Result<TrustStore, ConfigError> {
    store::load(path)
}

/// Changes the trust store at `path` with `change` under an exclusive file lock
/// (`trust.json.lock` beside it; several instances may decide at once) and writes it back
/// through a temporary file; answers the store as written. A store that does not load is not
/// overwritten.
pub fn update_trust_store(
    path: &Path,
    change: impl FnOnce(&mut TrustStore),
) -> Result<TrustStore, ConfigError> {
    store::update(path, change)
}

/// The trust of the project at `root`, against `store`: its protected parts, their hash, and
/// whether a record of `store` covers this root with this hash.
///
/// The protected values are read from the project files themselves, not from an effective
/// config: the loader removes the trust-gated keys from it, and the headers of both files are
/// protected. Only reading and parsing the files can fail here; whether the config validates
/// does not count.
pub fn trust_state(root: &Path, store: &TrustStore) -> Result<TrustState, ConfigError> {
    let raw = crate::load::read_files(root)?;
    state_of(root, &raw, store)
}

/// Loads the project at `root` like [`crate::load_project`] and decides its trust against
/// `store` from the same reading of the files: in a trusted workspace the trust-gated network
/// keys of `config.json` and `config.local.json` apply; otherwise they are removed with a
/// warning each, as by [`crate::load_project`].
pub fn load_project_trusted(
    root: &Path,
    store: &TrustStore,
) -> Result<(LoadedConfig, TrustState), ConfigError> {
    let raw = crate::load::read_files(root)?;
    let state = state_of(root, &raw, store)?;
    let loaded = crate::load::finish(root, raw, state.trusted)?;
    Ok((loaded, state))
}

/// Loads the project at `root` like [`load_project_trusted`] for a trust already decided by an
/// earlier check (`trusted`; the shell keeps it), without hashing the protected parts again.
/// For readers that show the config, never for a trust decision.
pub fn load_project_with_trust(root: &Path, trusted: bool) -> Result<LoadedConfig, ConfigError> {
    crate::load::finish(root, crate::load::read_files(root)?, trusted)
}

fn state_of(
    root: &Path,
    raw: &crate::load::RawFiles,
    store: &TrustStore,
) -> Result<TrustState, ConfigError> {
    let protected = hash::protected(root, raw)?;
    let trusted = protected.unpinned.is_empty()
        && store
            .record(root)
            .is_some_and(|record| record.hash == protected.hash);
    Ok(TrustState {
        trusted,
        hash: protected.hash,
        blocked: protected.blocked,
        unpinned: protected.unpinned,
    })
}

#[cfg(test)]
#[path = "trust_tests.rs"]
mod tests;
