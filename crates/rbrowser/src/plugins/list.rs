//! The installed plugins (RB-080-51): one row per package the start found - loaded, switched
//! off (`plugins.disabled`) or refused with its reason - for Settings > Plugins
//! (`pluginPanel { op: "installed" }`) and MCP's `plugins_list` (the command `plugins.list`),
//! both from [`rows`], so they name the same fields.
//!
//! A verified package's row carries what its manifest says (name, version, description - shown
//! as text, never as HTML), where it comes from, its signature, whether it runs and why not. A
//! **refused** package has no identity: its row is named by its file, `verified` is `false`, and
//! what its manifest claims ([`Declared`], read once at the load) reaches only the settings
//! window, marked unverified - never MCP ([`for_mcp`]), and never the id a switch writes. A
//! project package of an untrusted workspace is not read at all. No row carries a public key or
//! a path; the window alone also gets the folders the start read ([`folders`]). Nothing here
//! installs, enables or trusts a plugin.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rb_plugin_host::{
    DisabledEntry, HostServices as _, PluginManifest, PluginSource, Refused, project_plugins_dir,
};
use rb_plugin_types::{Capability, Failure, FailureKind, PluginKind, codes};
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::commands::host_failure_message;
use super::runtime::may_run;
use crate::analyzer::plugins::LoadedPlugins;

/// What the signature check of a package gave.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureState {
    /// Signed with a trusted key.
    Signed,
    /// No signature: loaded only in development mode.
    Unsigned,
    /// Changed after it was signed, or signed with another key than the one trusted.
    Invalid,
    /// Signed with a key nobody confirmed.
    Untrusted,
    /// Refused before its signature was checked.
    Unknown,
}

/// The signature of a row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub state: SignatureState,
    /// The id of the signing key the manifest names (an id, never the key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
}

/// Whether a plugin runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RowState {
    /// Loaded and running.
    Active,
    /// Switched off in `plugins.disabled`.
    Disabled,
    /// Loaded, paused after repeated failures (`plugins.crashLimit`).
    Suspended,
    /// A project plugin while the workspace is not trusted.
    Blocked,
    /// The package did not load.
    Error,
}

/// Why a plugin did not load or does not run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RowError {
    /// The stable code (`plugins.error.*`).
    pub code: String,
    /// The catalogue's text with the failure's parameters; left out over MCP for a refused
    /// package, whose parameters may quote the package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// What a refused package's manifest says of itself: unverified text, for a person only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Declared {
    pub name: String,
    pub version: String,
    /// The name of the admitted plugin whose id the package claims: it is another package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claims_id_of: Option<String>,
}

/// One installed plugin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRow {
    /// The id of a verified package; empty for a refused one, which has no identity.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The package's file name: what names a refused package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Whether the package verified: only then are `id` and the manifest's texts its own.
    pub verified: bool,
    /// Unverified, for the settings window only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared: Option<Declared>,
    pub source: PluginSource,
    pub signature: Signature,
    pub state: RowState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RowError>,
    pub kinds: Vec<PluginKind>,
    pub capabilities: Vec<Capability>,
    pub mcp_prefix: Option<String>,
    /// Its MCP tools, `<mcp_prefix>_<tool>`; none unless it is loaded.
    pub tools: Vec<String>,
    /// How many commands it contributes (`plugin.<id>.*`); none unless it is loaded.
    pub commands: usize,
    /// How many settings keys it contributes; none unless it is loaded.
    pub settings: usize,
}

/// Whether a loaded plugin runs now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Running {
    Yes,
    /// Paused after repeated failures.
    Suspended,
    /// A project plugin, and the workspace is not trusted now.
    Untrusted,
}

/// The text of `failure`: the catalogue's, else its code and parameters.
fn error_of(failure: &Failure) -> RowError {
    RowError {
        code: failure.code().unwrap_or(codes::HOST_FAILED).to_owned(),
        message: Some(host_failure_message(failure)),
    }
}

fn coded(kind: FailureKind, code: &str) -> RowError {
    error_of(&Failure::coded(kind, code))
}

/// The row of a manifest: what it says about itself, nothing it contributes yet.
fn manifest_row(
    source: PluginSource,
    manifest: &PluginManifest,
    signature: SignatureState,
    state: RowState,
) -> PluginRow {
    PluginRow {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.to_string(),
        description: manifest.description.clone(),
        file: None,
        verified: true,
        declared: None,
        source,
        signature: Signature {
            state: signature,
            key_id: (signature != SignatureState::Unsigned).then(|| manifest.key_id.clone()),
        },
        state,
        error: None,
        kinds: manifest.kinds.clone(),
        capabilities: manifest.grants.granted.iter().copied().collect(),
        mcp_prefix: manifest.mcp_prefix.clone(),
        tools: Vec::new(),
        commands: 0,
        settings: 0,
    }
}

fn signature_of(signed: bool) -> SignatureState {
    if signed {
        SignatureState::Signed
    } else {
        SignatureState::Unsigned
    }
}

/// The row of a loaded plugin with `settings` keys of its own.
pub fn loaded_row(
    source: PluginSource,
    manifest: &PluginManifest,
    signed: bool,
    running: Running,
    settings: usize,
) -> PluginRow {
    let (state, error) = match running {
        Running::Yes => (RowState::Active, None),
        Running::Suspended => (
            RowState::Suspended,
            Some(coded(FailureKind::Other, codes::SUSPENDED)),
        ),
        Running::Untrusted => (
            RowState::Blocked,
            Some(coded(FailureKind::AccessDenied, codes::UNTRUSTED_WORKSPACE)),
        ),
    };
    PluginRow {
        error,
        tools: manifest
            .tools
            .iter()
            .filter_map(|tool| manifest.tool_name(&tool.name))
            .collect(),
        commands: manifest.commands.len(),
        settings,
        ..manifest_row(source, manifest, signature_of(signed), state)
    }
}

/// The row of a package switched off in `plugins.disabled`.
pub fn disabled_row(entry: &DisabledEntry) -> PluginRow {
    manifest_row(
        entry.source,
        &entry.manifest,
        signature_of(entry.signed),
        RowState::Disabled,
    )
}

/// The row of a package that did not load: named by its file, never by what it claims.
/// `admitted` are the verified plugins by id with their names: a refused package claiming one
/// of those ids is said to claim it, and stays a row of its own.
pub fn refused_row(refused: &Refused, admitted: &BTreeMap<String, String>) -> PluginRow {
    let code = refused.failure.code().unwrap_or_default();
    let signature = match code {
        codes::UNSIGNED => SignatureState::Unsigned,
        codes::SIGNATURE_INVALID | codes::KEY_MISMATCH => SignatureState::Invalid,
        codes::KEY_UNTRUSTED => SignatureState::Untrusted,
        _ => refused.signed.map_or(SignatureState::Unknown, signature_of),
    };
    let state = if code == codes::UNTRUSTED_WORKSPACE {
        RowState::Blocked
    } else {
        RowState::Error
    };
    PluginRow {
        id: String::new(),
        name: String::new(),
        version: String::new(),
        description: String::new(),
        file: Some(
            refused
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
        verified: false,
        declared: refused.claims.as_deref().map(|claims| Declared {
            name: claims.name.clone(),
            version: claims.version.to_string(),
            claims_id_of: admitted.get(&claims.id).cloned(),
        }),
        source: refused.source,
        signature: Signature {
            state: signature,
            key_id: None,
        },
        state,
        error: Some(error_of(&refused.failure)),
        kinds: Vec::new(),
        capabilities: Vec::new(),
        mcp_prefix: None,
        tools: Vec::new(),
        commands: 0,
        settings: 0,
    }
}

/// `rows` as an agent gets them: of a refused package the file name, source, state and error
/// code only - no text its manifest claims, no failure parameters (they may quote the package).
pub fn for_mcp(mut rows: Vec<PluginRow>) -> Vec<PluginRow> {
    for row in rows.iter_mut().filter(|row| !row.verified) {
        row.declared = None;
        if let Some(error) = &mut row.error {
            error.message = None;
        }
    }
    rows
}

/// Bundled first, then the person's, then the project's; within a source the verified plugins
/// by id, then the refused packages by file.
pub fn sort(rows: &mut [PluginRow]) {
    let rank = |source: PluginSource| match source {
        PluginSource::Bundled => 0,
        PluginSource::User => 1,
        PluginSource::Project => 2,
    };
    rows.sort_by(|a, b| {
        (rank(a.source), !a.verified, &a.id, &a.file).cmp(&(
            rank(b.source),
            !b.verified,
            &b.id,
            &b.file,
        ))
    });
}

/// Every package of `loaded`'s start as a row; `project_trusted` is the workspace trust now.
/// Reads no file: what a refused package claims was read at the load.
pub fn rows(loaded: &LoadedPlugins, project_trusted: bool) -> Vec<PluginRow> {
    let report = &loaded.report;
    let mut rows: Vec<PluginRow> = report
        .loaded
        .iter()
        .map(|entry| {
            let manifest = entry.plugin.manifest();
            let running = if !may_run(entry.source, project_trusted) {
                Running::Untrusted
            } else if entry.plugin.health().is_suspended() {
                Running::Suspended
            } else {
                Running::Yes
            };
            loaded_row(
                entry.source,
                manifest,
                entry.signed,
                running,
                loaded
                    .settings_counts
                    .get(&manifest.id)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect();
    rows.extend(report.disabled.iter().map(disabled_row));
    let admitted: BTreeMap<String, String> = rows
        .iter()
        .map(|row| (row.id.clone(), row.name.clone()))
        .collect();
    rows.extend(
        report
            .refused
            .iter()
            .map(|refused| refused_row(refused, &admitted)),
    );
    sort(&mut rows);
    rows
}

/// The answer of `plugins.list` and of the window's request: `rows`, or `loading` while the
/// start still loads the plugins (`None`).
pub fn answer(rows: Option<Vec<PluginRow>>) -> Value {
    json!({
        "loading": rows.is_none(),
        "plugins": rows.unwrap_or_default(),
    })
}

/// What the window's answer says of the folders: `folders` names, per source, the folder the
/// start read (it exists), `missing` the one it looked for and did not find.
pub fn folders_value(
    candidates: [(&str, Option<PathBuf>); 3],
    exists: impl Fn(&PathBuf) -> bool,
) -> Value {
    let mut found = Map::new();
    let mut missing = Map::new();
    for (source, folder) in candidates {
        if let Some(folder) = folder {
            let text = Value::String(folder.display().to_string());
            if exists(&folder) {
                found.insert(source.to_owned(), text);
            } else {
                missing.insert(source.to_owned(), text);
            }
        }
    }
    json!({ "folders": found, "missing": missing })
}

/// The folders this process reads plugins from - beside the executable, beside the user
/// settings, the project's `.rbrowser/plugins/` - each as read or as not found. Looks at the
/// disk: not for the UI thread.
pub fn folders() -> Value {
    folders_value(
        [
            ("bundled", crate::analyzer::plugins::bundled_dir()),
            ("user", crate::analyzer::plugins::user_dir()),
            (
                "project",
                crate::analyzer::plugins::project_root().map(|root| project_plugins_dir(&root)),
            ),
        ],
        |folder| folder.is_dir(),
    )
}

/// Whether the open workspace is trusted now: what a project plugin needs to run.
pub fn trusted() -> bool {
    super::services::shared().workspace_trusted()
}

/// The rows of this process's plugins once they are loaded; `None` while the load runs. Never
/// waits for the load.
pub fn current() -> Option<Vec<PluginRow>> {
    crate::analyzer::plugins::loaded().map(|loaded| rows(&loaded, trusted()))
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
