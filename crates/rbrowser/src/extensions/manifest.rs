//! An unpacked extension's `manifest.json`: Manifest V3 only, a name, a version Chromium
//! accepts, the id when the manifest carries its `key`, and no `debugger` permission - that
//! one would let the extension drive every CDP target, rBrowser's own pages included.

use base64::Engine as _;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

/// The manifest's file name in an extension folder.
pub const MANIFEST_FILE: &str = "manifest.json";
/// The largest manifest read; a real one is a few KiB.
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
/// Permissions an extension in rBrowser may not ask for.
pub const REFUSED_PERMISSIONS: &[&str] = &["debugger"];

/// What a valid manifest tells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// `name`, as written (a `__MSG_...__` name stays one).
    pub name: String,
    /// `version`.
    pub version: String,
    /// The extension id, from `key`; `None` without one (Chromium derives it from the path).
    pub id: Option<String>,
}

/// Why a manifest is refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    /// Not a JSON object.
    #[error("manifest.json is not a JSON object: {0}")]
    NotJson(String),
    /// `manifest_version` is not 3 (what it is instead).
    #[error("manifest.json declares manifest_version {0}; rBrowser loads Manifest V3 only")]
    NotV3(String),
    /// No `name`.
    #[error("manifest.json has no name")]
    NoName,
    /// A version Chromium refuses.
    #[error("manifest.json has the version \"{0}\", which is not one to four numbers 0-65535")]
    BadVersion(String),
    /// A `key` that is no base64.
    #[error("manifest.json has a key that is not base64")]
    BadKey,
    /// A permission rBrowser refuses.
    #[error(
        "the extension asks for the permission \"{0}\", which would let it control rBrowser's \
         own pages; rBrowser does not load it"
    )]
    Refused(&'static str),
}

impl ManifestError {
    /// The stable code of the error (MCP answers, notices).
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotJson(_) => "extensions.manifest_invalid",
            Self::NotV3(_) => "extensions.manifest_not_v3",
            Self::NoName => "extensions.manifest_no_name",
            Self::BadVersion(_) => "extensions.manifest_bad_version",
            Self::BadKey => "extensions.manifest_bad_key",
            Self::Refused(_) => "extensions.permission_refused",
        }
    }
}

/// Reads and checks the text of a `manifest.json`.
///
/// # Errors
///
/// The first rule the manifest breaks.
pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| ManifestError::NotJson(error.to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| ManifestError::NotJson("the top level is no object".to_owned()))?;
    match object.get("manifest_version") {
        Some(Value::Number(number)) if number.as_u64() == Some(3) => {}
        Some(other) => return Err(ManifestError::NotV3(other.to_string())),
        None => return Err(ManifestError::NotV3("none".to_owned())),
    }
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or(ManifestError::NoName)?
        .to_owned();
    let version = match object.get("version") {
        Some(Value::String(version)) if is_valid_version(version) => version.clone(),
        Some(Value::String(version)) => return Err(ManifestError::BadVersion(version.clone())),
        other => {
            return Err(ManifestError::BadVersion(
                other.map_or_else(String::new, Value::to_string),
            ));
        }
    };
    for list in ["permissions", "optional_permissions"] {
        let asked = object.get(list).and_then(Value::as_array);
        for permission in asked.into_iter().flatten().filter_map(Value::as_str) {
            if let Some(refused) = REFUSED_PERMISSIONS.iter().find(|r| **r == permission) {
                return Err(ManifestError::Refused(refused));
            }
        }
    }
    let id = match object.get("key") {
        Some(Value::String(key)) => Some(id_from_key(key).ok_or(ManifestError::BadKey)?),
        Some(_) => return Err(ManifestError::BadKey),
        None => None,
    };
    Ok(Manifest { name, version, id })
}

/// The extension id of the public key `key` (base64 of its DER): the first 16 bytes of its
/// SHA-256, each hex digit written as `a` to `p`. `None` when `key` is no base64.
pub fn id_from_key(key: &str) -> Option<String> {
    let compact: String = key.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let der = base64::engine::general_purpose::STANDARD
        .decode(compact)
        .ok()?;
    Some(id_from_der(&der))
}

/// The extension id of the DER public key `der`.
pub fn id_from_der(der: &[u8]) -> String {
    let digest = Sha256::digest(der);
    digest
        .iter()
        .take(16)
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .map(|nibble| char::from(b'a' + nibble))
        .collect()
}

/// Whether `version` is one Chromium accepts: one to four dot-separated numbers, each 0 to
/// 65535 and without a leading zero.
pub fn is_valid_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    (1..=4).contains(&parts.len())
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
                && part.parse::<u32>().is_ok_and(|n| n <= 65_535)
        })
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
