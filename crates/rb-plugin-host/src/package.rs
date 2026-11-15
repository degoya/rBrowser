//! The `.rbplugin` package: its members, their size limits, the digest its signature covers, and
//! reading one with every member bounded (`docs/plugins.md`, "Packages, signing and trust").
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host/src/package.rs` (itself from
//! rDownloader's): a ZIP, every member bounded before it is read, anything unexpected refusing the
//! package. rBrowser's package carries more than a component - `settings.schema.json`, the
//! locales, a page script under `page/`, a panel under `panel/` - and **the signature covers every
//! file**: manifest and component as fields, every other member by name and content.

use std::collections::BTreeSet;
use std::io::{Read, Seek};

use anyhow::{Context, Result, bail};
use rb_plugin_types::paths;

use crate::manifest::{MAX_MANIFEST_BYTES, PluginManifest, SETTINGS_SCHEMA_MEMBER};
use crate::sign::DigestBuilder;

/// The file name extension of a package.
pub const PACKAGE_EXTENSION: &str = "rbplugin";
pub const MANIFEST_MEMBER: &str = "manifest.toml";
pub const COMPONENT_MEMBER: &str = "component.wasm";
pub const SIGNATURE_MEMBER: &str = "signature.ed25519";
/// The largest component a package holds.
pub const MAX_COMPONENT_BYTES: u64 = 64 * 1024 * 1024;
/// The largest signature member: base64 of 64 bytes is 88 characters.
pub const MAX_SIGNATURE_BYTES: u64 = 1024;
/// The largest settings schema or locale file.
pub const MAX_TEXT_MEMBER_BYTES: u64 = 256 * 1024;
/// The largest single page or panel file.
pub const MAX_ASSET_BYTES: u64 = 4 * 1024 * 1024;
/// The most members besides manifest, component and signature.
pub const MAX_FILES: usize = 256;
/// The most bytes of those members together.
pub const MAX_FILES_BYTES: u64 = 32 * 1024 * 1024;
/// The largest package file read at all.
pub const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;

/// A package's members, read and checked: what an install writes and a start loads.
#[derive(Clone)]
pub struct VerifiedPackage {
    pub manifest: PluginManifest,
    pub manifest_bytes: Vec<u8>,
    pub component: Vec<u8>,
    /// `None` for an unsigned package, which only development mode accepts.
    pub signature: Option<Vec<u8>>,
    /// Every other member, `(name, bytes)`, sorted by name, exactly as signed.
    pub files: Vec<(String, Vec<u8>)>,
}

impl VerifiedPackage {
    /// [`package_digest`] of its members.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        package_digest(&self.manifest_bytes, &self.component, &self.files)
    }

    /// The member `name`, if the package has it.
    #[must_use]
    pub fn file(&self, name: &str) -> Option<&[u8]> {
        self.files
            .iter()
            .find(|(member, _)| member == name)
            .map(|(_, bytes)| bytes.as_slice())
    }

    /// The members a page or a panel loads, under `page/` and `panel/` (RB-060-12).
    #[must_use]
    pub fn ui_files(&self) -> std::collections::BTreeMap<String, Vec<u8>> {
        self.files
            .iter()
            .filter(|(member, _)| member.starts_with("page/") || member.starts_with("panel/"))
            .cloned()
            .collect()
    }

    /// `settings.schema.json`, when the manifest declares settings.
    #[must_use]
    pub fn settings_schema(&self) -> Option<&[u8]> {
        self.file(SETTINGS_SCHEMA_MEMBER)
    }

    /// The locale files, `(language, bytes)`, as `[locales]` names them.
    #[must_use]
    pub fn locales(&self) -> Vec<(&str, &[u8])> {
        self.manifest
            .locales
            .iter()
            .filter_map(|(language, member)| {
                self.file(member).map(|bytes| (language.as_str(), bytes))
            })
            .collect()
    }
}

impl std::fmt::Debug for VerifiedPackage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedPackage")
            .field("id", &self.manifest.id)
            .field("version", &self.manifest.version.to_string())
            .field("component_bytes", &self.component.len())
            .field("files", &self.files.len())
            .field("signed", &self.signature.is_some())
            .finish_non_exhaustive()
    }
}

/// The members of a package: manifest, component, signature, the other files.
pub(crate) struct PackageParts {
    pub(crate) manifest: Vec<u8>,
    pub(crate) component: Vec<u8>,
    pub(crate) signature: Option<Vec<u8>>,
    pub(crate) files: Vec<(String, Vec<u8>)>,
}

/// Where a member other than the three fixed ones may live, and how large it may be.
fn file_limit(name: &str) -> Option<u64> {
    if name == SETTINGS_SCHEMA_MEMBER {
        return Some(MAX_TEXT_MEMBER_BYTES);
    }
    let segments = paths::relative_segments(name).ok()?;
    if name.contains('\\') {
        return None;
    }
    match segments.as_slice() {
        ["locales", file] if file.ends_with(".json") => Some(MAX_TEXT_MEMBER_BYTES),
        ["page" | "panel", _, ..] => Some(MAX_ASSET_BYTES),
        _ => None,
    }
}

/// Reads the members of a `.rbplugin`, refusing an unexpected member or a member over its limit.
pub(crate) fn read_archive<R: Read + Seek>(reader: R) -> Result<PackageParts> {
    let mut archive = zip::ZipArchive::new(reader).context("read .rbplugin archive")?;
    let mut names: Vec<String> = archive
        .file_names()
        .map(|name| name.map(std::borrow::Cow::into_owned))
        .collect::<Result<_, _>>()
        .context("read .rbplugin member names")?;
    // A folder entry carries nothing; every file is read by its name, so what is verified is
    // what is read.
    names.retain(|name| !name.ends_with('/'));
    let mut files = Vec::new();
    let mut total = 0_u64;
    for name in &names {
        match name.as_str() {
            MANIFEST_MEMBER | COMPONENT_MEMBER | SIGNATURE_MEMBER => {}
            other => {
                let limit = file_limit(other)
                    .with_context(|| format!("unexpected member {other} in .rbplugin archive"))?;
                let bytes = read_member(&mut archive, other, limit)?;
                total += bytes.len() as u64;
                files.push((other.to_owned(), bytes));
            }
        }
    }
    if files.len() > MAX_FILES || total > MAX_FILES_BYTES {
        bail!("the package carries more files than a plugin may");
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(PackageParts {
        manifest: read_member(&mut archive, MANIFEST_MEMBER, MAX_MANIFEST_BYTES as u64)?,
        component: read_member(&mut archive, COMPONENT_MEMBER, MAX_COMPONENT_BYTES)?,
        signature: read_optional_member(&mut archive, SIGNATURE_MEMBER, MAX_SIGNATURE_BYTES)?,
        files,
    })
}

/// The members a manifest names are there, and the locales and the settings schema are exactly
/// those it declares.
pub(crate) fn check_members(manifest: &PluginManifest, files: &[(String, Vec<u8>)]) -> Result<()> {
    let present: BTreeSet<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    for member in manifest.named_members() {
        if !present.contains(member) {
            bail!("the manifest names {member}, which the package does not carry");
        }
    }
    for name in &present {
        if name.starts_with("locales/") && !manifest.locales.values().any(|member| member == name) {
            bail!("{name} is no locale the manifest declares");
        }
    }
    if present.contains(SETTINGS_SCHEMA_MEMBER) != manifest.settings {
        bail!("{SETTINGS_SCHEMA_MEMBER} and the manifest's [settings] go together");
    }
    Ok(())
}

/// The exact payload a package signature covers: manifest and component length-prefixed, every
/// other member folded in by name, so a translation, a page script or a panel is as
/// tamper-evident as the component.
#[must_use]
pub fn package_digest(manifest: &[u8], component: &[u8], files: &[(String, Vec<u8>)]) -> [u8; 32] {
    let mut builder = DigestBuilder::new();
    builder.field(manifest);
    builder.field(component);
    builder.named_fields(files);
    builder.finish()
}

fn read_member<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>> {
    read_optional_member(archive, name, limit)?.with_context(|| format!("missing {name}"))
}

fn read_optional_member<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Option<Vec<u8>>> {
    let member = match archive.by_name(name) {
        Ok(member) => member,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if member.size() > limit {
        bail!("plugin member {name} exceeds its size limit");
    }
    // The header's size is the archive's word; the read is held to the limit whatever it says.
    let mut bytes = Vec::with_capacity(usize::try_from(member.size()).unwrap_or(0));
    member.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("plugin member {name} exceeds its size limit");
    }
    Ok(Some(bytes))
}

#[cfg(test)]
#[path = "package_tests.rs"]
mod tests;
