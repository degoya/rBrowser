//! The ZIP inside a verified CRX ([`super`]): the manifest read out of it for the prompt
//! without unpacking, and the unpacking into the staging folder - every member name checked
//! (no absolute path, no `..`, no drive, no backslash, no two names alike in letter case), the
//! members and bytes held to the caller's [`Limits`].

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};

use super::{InstallError, Limits, io};
use crate::extensions::folder::FolderError;
use crate::extensions::manifest::MANIFEST_FILE;

/// The `manifest.json` inside `zip`, checked as an unpacked folder's would be, without
/// unpacking anything (the prompt names it). The member names are checked for a collision
/// first, so the prompt never names a manifest another member would overwrite.
///
/// # Errors
///
/// No ZIP, no manifest, colliding names, or a manifest rBrowser refuses.
pub fn peek_manifest(zip: &[u8]) -> Result<crate::extensions::manifest::Manifest, InstallError> {
    let unpack_failed = |error: zip::result::ZipError| InstallError::Unpack(error.to_string());
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).map_err(unpack_failed)?;
    check_names(&archive)?;
    let mut member = match archive.by_name(MANIFEST_FILE) {
        Ok(member) => member,
        Err(zip::result::ZipError::FileNotFound) => {
            return Err(InstallError::Manifest(FolderError::NoManifest));
        }
        Err(error) => return Err(unpack_failed(error)),
    };
    let mut text = String::new();
    (&mut member)
        .take(crate::extensions::manifest::MAX_MANIFEST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|error| InstallError::Unpack(error.to_string()))?;
    if u64::try_from(text.len()).unwrap_or(u64::MAX)
        > crate::extensions::manifest::MAX_MANIFEST_BYTES
    {
        return Err(InstallError::Manifest(FolderError::TooLarge));
    }
    Ok(
        crate::extensions::manifest::parse(text.trim_start_matches('\u{feff}'))
            .map_err(FolderError::Manifest)?,
    )
}

/// Unpacks `zip` into `dir` within `limits`; every member path checked.
pub(super) fn unpack(zip: &[u8], dir: &Path, limits: Limits) -> Result<(), InstallError> {
    let unpack_failed = |error: zip::result::ZipError| InstallError::Unpack(error.to_string());
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).map_err(unpack_failed)?;
    if archive.len() > limits.files {
        return Err(InstallError::TooManyFiles(limits.files));
    }
    check_names(&archive)?;
    let mut budget = limits.bytes;
    for index in 0..archive.len() {
        let mut member = archive.by_index(index).map_err(unpack_failed)?;
        let name = member.name().map_err(unpack_failed)?.into_owned();
        let relative = safe_path(&name).ok_or_else(|| InstallError::UnsafePath(name.clone()))?;
        let path = dir.join(relative);
        if member.is_dir() {
            fs::create_dir_all(&path).map_err(io)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        let mut out = File::create(&path).map_err(io)?;
        let copied = std::io::copy(&mut (&mut member).take(budget.saturating_add(1)), &mut out)
            .map_err(|error| InstallError::Unpack(error.to_string()))?;
        if copied > budget {
            return Err(InstallError::TooLarge(limits.bytes));
        }
        budget -= copied;
    }
    Ok(())
}

/// Refuses two member names that are one name once letter case is folded (`Manifest.json`
/// beside `manifest.json`): on Windows the second would overwrite the first.
fn check_names<R: std::io::Read + std::io::Seek>(
    archive: &zip::ZipArchive<R>,
) -> Result<(), InstallError> {
    let mut seen = BTreeSet::new();
    for name in archive.file_names() {
        let name = name.map_err(|error| InstallError::Unpack(error.to_string()))?;
        let folded = name.trim_end_matches('/').to_lowercase();
        if !seen.insert(folded) {
            return Err(InstallError::NameCollision(name.into_owned()));
        }
    }
    Ok(())
}

/// `name` as a path inside the folder: plain segments only, no absolute path, no drive, no
/// `..` or `.`, no backslash; a trailing `/` (a folder) is allowed.
fn safe_path(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.starts_with('/') || name.contains('\\') || name.contains('\0') {
        return None;
    }
    let mut path = PathBuf::new();
    for segment in name.trim_end_matches('/').split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.contains(':')
            || segment.ends_with(' ')
            || segment.ends_with('.')
        {
            return None;
        }
        path.push(segment);
    }
    (!path.as_os_str().is_empty()).then_some(path)
}
