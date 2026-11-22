//! The settings keys MCP may not write for every installed plugin package - bundled (chosen or
//! not), the person's and the project's - read from the package's manifest and
//! `settings.schema.json` without verifying or loading it (RB-094-31). The settings registry
//! knows a plugin's keys only while it is loaded; a protected key (`x-protected`: allowed hosts,
//! secrets) `config_set` wrote while its plugin was not loaded would be read as the person's at
//! the next load. A package whose keys do not read is guarded whole: its id's prefix, from the
//! manifest or - when that does not read either - from the file name.
//!
//! What is read here only ever adds a refusal: nothing of it is trusted, granted or shown.

use std::collections::BTreeSet;
use std::io::{Read, Seek};
use std::path::Path;

use rb_plugin_host::{
    MANIFEST_MEMBER, MAX_MANIFEST_BYTES, MAX_TEXT_MEMBER_BYTES, RESERVED_IDS,
    SETTINGS_SCHEMA_MEMBER, Sources, check_settings_schema, discover, parse_manifest,
    valid_plugin_id,
};

/// The dotted keys MCP may not write for the packages of `sources` (the loader's folders and
/// order, `rb_plugin_host::discover`): each package's protected keys, or its whole prefix when
/// they do not read.
#[must_use]
pub fn protected_keys(sources: &Sources) -> BTreeSet<String> {
    discover(sources)
        .into_iter()
        .flat_map(|(_, path)| guarded_by(&path))
        .collect()
}

/// What the package at `path` guards: the keys its schema marks `x-protected`; nothing when its
/// manifest declares no settings; its id when the schema does not read; the id its file name
/// names when the manifest does not read.
fn guarded_by(path: &Path) -> Vec<String> {
    let archive = std::fs::File::open(path)
        .ok()
        .and_then(|file| zip::ZipArchive::new(file).ok());
    let Some(mut archive) = archive else {
        return named_by_file(path);
    };
    let manifest = member(&mut archive, MANIFEST_MEMBER, MAX_MANIFEST_BYTES)
        .and_then(|bytes| parse_manifest(&bytes).ok());
    let Some(manifest) = manifest else {
        return named_by_file(path);
    };
    if !manifest.settings {
        return Vec::new();
    }
    let limit = usize::try_from(MAX_TEXT_MEMBER_BYTES).unwrap_or(usize::MAX);
    let defs = member(&mut archive, SETTINGS_SCHEMA_MEMBER, limit)
        .and_then(|bytes| check_settings_schema(&bytes).ok())
        .and_then(|schema| rb_settings::plugin_defs_from_schema(&manifest.id, &schema).ok());
    match defs {
        Some(defs) => defs
            .into_iter()
            .filter(|def| def.protected_from_mcp)
            .map(|def| def.key)
            .collect(),
        None => vec![manifest.id],
    }
}

/// The member `name` of `archive`, when it is there and holds at most `limit` bytes.
fn member<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: usize,
) -> Option<Vec<u8>> {
    let entry = archive.by_name(name).ok()?;
    let mut bytes = Vec::new();
    let most = u64::try_from(limit.saturating_add(1)).unwrap_or(u64::MAX);
    entry.take(most).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= limit).then_some(bytes)
}

/// The plugin ids the file name of the package at `path` names: `<id>.rbplugin` as an install
/// writes it, and `<id>-<version>.rbplugin` as a release names it.
fn named_by_file(path: &Path) -> Vec<String> {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return Vec::new();
    };
    let unversioned = stem
        .rsplit_once('-')
        .filter(|(_, version)| version.starts_with(|c: char| c.is_ascii_digit()))
        .map(|(id, _)| id);
    [Some(stem), unversioned]
        .into_iter()
        .flatten()
        .filter(|id| valid_plugin_id(id) && !RESERVED_IDS.contains(id))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
#[path = "installed_keys_tests.rs"]
mod tests;
