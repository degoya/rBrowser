//! The built-in manifest of the managed tools (`tools.json`, compiled in): per tool and
//! platform the pinned version, the download address, its SHA-256 and size, the archive members
//! taken, the licence and the address of its source code (the LGPL's offer). It names `ffmpeg`
//! and `adb` (the Android Platform Tools, RB-095-02) and `cloudflared` (RB-096-05, the release's
//! bare program) for `x86_64-pc-windows-msvc`.
//!
//! The manifest is held to its rules on every read, so a malformed pin can never reach the
//! network: https only, a real SHA-256 (64 lowercase hex digits, never [`PLACEHOLDER_SHA256`]),
//! a size up to [`MAX_SIZE`], an LGPL or Apache-2.0 licence ([`LICENSES`]; for `adb` the Android
//! SDK licence its binaries come under, [`ANDROID_SDK_LICENSE`]), and names that are plain folder
//! and file names; a bare program ([`Archive::Exe`]) has its executable as its only member.
//!
//! The addresses: BtbN keeps a daily build for two weeks but the last build of each month for
//! two years, so `url` names a month-end `autobuild-<YYYY-MM-DD>-…` release (as rDownloader
//! does, RD-1101-13; a test holds it) and `source_url` the matching FFmpeg commit on GitHub.
//! `upstream_url` and `upstream_source_url` are for a mirror, should one be needed again.
//! `adb` is Google's versioned `platform-tools_r<version>-win.zip`, which stays served after
//! newer releases; `source_url` is adb's source (Apache-2.0). `scripts/tools-manifest-check.sh`
//! asks every address.

use serde::{Deserialize, Serialize};

use super::ToolError;

/// The manifest compiled into the app.
pub const EMBEDDED: &str = include_str!("tools.json");
/// The only manifest format.
pub const FORMAT_VERSION: u32 = 1;
/// A pin that was never set: refused like any other malformed digest.
pub const PLACEHOLDER_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
/// The largest download any entry may declare.
pub const MAX_SIZE: u64 = 512 * 1024 * 1024;
/// The platform name of Windows x64.
pub const WINDOWS_X64: &str = "x86_64-pc-windows-msvc";
/// The video encoder.
pub const FFMPEG: &str = "ffmpeg";
/// The Android Debug Bridge of the Platform Tools (RB-095-02).
pub const ADB: &str = "adb";
/// The licence of Google's Platform Tools binaries (no SPDX id): taken for [`ADB`] only.
pub const ANDROID_SDK_LICENSE: &str = "LicenseRef-Android-SDK-License";
/// The tunnel of the portal's presentation over the internet (RB-096-05).
pub const CLOUDFLARED: &str = "cloudflared";
/// The licences besides an LGPL (`LGPL-*`) a managed tool may have; never a GPL.
pub const LICENSES: &[&str] = &["Apache-2.0"];
/// The longest version or file name taken.
const MAX_NAME: usize = 64;

/// The archive formats taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Archive {
    /// A ZIP file.
    Zip,
    /// The program itself (cloudflared's release asset): it becomes the executable.
    Exe,
}

/// One tool on one platform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolEntry {
    /// The tool (`ffmpeg`): its folder below the tools folder.
    pub name: String,
    /// The Rust target triple it runs on.
    pub platform: String,
    /// The pinned version: the folder it is installed to.
    pub version: String,
    /// Where the archive is downloaded from (https).
    pub url: String,
    /// The archive's SHA-256, lowercase hex.
    pub sha256: String,
    /// The archive's size in bytes.
    pub size: u64,
    /// The archive's format.
    pub archive: Archive,
    /// The program among the members.
    pub executable: String,
    /// The file names taken from the archive, wherever they lie in it.
    pub members: Vec<String>,
    /// The SPDX licence of the build.
    pub license: String,
    /// Where its source code is (https): the LGPL's offer.
    pub source_url: String,
    /// Where the mirrored archive came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_url: Option<String>,
    /// Where the mirrored source came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_source_url: Option<String>,
}

/// The manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// [`FORMAT_VERSION`].
    pub version: u32,
    /// Every tool on every platform.
    pub tools: Vec<ToolEntry>,
}

impl Manifest {
    /// Reads and checks a manifest.
    ///
    /// # Errors
    ///
    /// Not JSON of this format, another version, or an entry that breaks a rule.
    pub fn parse(text: &str) -> Result<Self, ToolError> {
        let manifest: Manifest =
            serde_json::from_str(text).map_err(|error| ToolError::Manifest(error.to_string()))?;
        if manifest.version != FORMAT_VERSION {
            return Err(ToolError::Manifest(format!(
                "version {} is not {FORMAT_VERSION}",
                manifest.version
            )));
        }
        for entry in &manifest.tools {
            entry
                .validate()
                .map_err(|reason| ToolError::Manifest(format!("{}: {reason}", entry.name)))?;
        }
        Ok(manifest)
    }

    /// The manifest compiled into the app.
    ///
    /// # Errors
    ///
    /// As [`Manifest::parse`]; a test holds the embedded one valid.
    pub fn embedded() -> Result<Self, ToolError> {
        Self::parse(EMBEDDED)
    }

    /// The entry of `name` on `platform`.
    ///
    /// # Errors
    ///
    /// [`ToolError::NoBuild`] when the manifest names none.
    pub fn entry(&self, name: &str, platform: &str) -> Result<&ToolEntry, ToolError> {
        self.tools
            .iter()
            .find(|entry| entry.name == name && entry.platform == platform)
            .ok_or_else(|| ToolError::NoBuild {
                tool: name.to_owned(),
                platform: platform.to_owned(),
            })
    }
}

impl ToolEntry {
    /// Checks the rules of the module documentation; the first one broken.
    ///
    /// # Errors
    ///
    /// Why the entry cannot be used.
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("name", &self.name),
            ("version", &self.version),
            ("executable", &self.executable),
        ] {
            if !valid_name(value) {
                return Err(format!("{field} {value:?} is no plain name"));
            }
        }
        if let Some(member) = self.members.iter().find(|member| !valid_name(member)) {
            return Err(format!("member {member:?} is no plain file name"));
        }
        if !self.members.contains(&self.executable) {
            return Err(format!("{} is not among the members", self.executable));
        }
        if self.archive == Archive::Exe && self.members.len() != 1 {
            return Err("a bare program has its executable as its only member".to_owned());
        }
        for (field, value) in [("url", &self.url), ("source_url", &self.source_url)] {
            if !value.starts_with("https://") || value.len() <= "https://".len() {
                return Err(format!("{field} {value} is not https"));
            }
        }
        if !valid_sha256(&self.sha256) {
            return Err(format!(
                "sha256 {:?} is no pinned SHA-256 (64 lowercase hex digits)",
                self.sha256
            ));
        }
        if self.size == 0 || self.size > MAX_SIZE {
            return Err(format!("size {} is not from 1 to {MAX_SIZE}", self.size));
        }
        let sdk_adb = self.name == ADB && self.license == ANDROID_SDK_LICENSE;
        if !self.license.starts_with("LGPL-")
            && !LICENSES.contains(&self.license.as_str())
            && !sdk_adb
        {
            return Err(format!(
                "licence {:?} is neither an LGPL nor one of {LICENSES:?}",
                self.license
            ));
        }
        Ok(())
    }
}

/// A real digest: 64 lowercase hex digits, not the placeholder.
fn valid_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f'))
        && text != PLACEHOLDER_SHA256
}

/// A plain folder or file name: ASCII letters, digits, `.`, `_`, `+`, `-`, not starting with a
/// dot, at most [`MAX_NAME`] characters - never a path.
pub fn valid_name(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_NAME
        && !text.starts_with('.')
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

/// The platform this build runs on, as the manifest names it; `None` where no tool is pinned.
pub fn current_platform() -> Option<&'static str> {
    (cfg!(windows) && cfg!(target_arch = "x86_64")).then_some(WINDOWS_X64)
}

/// `bytes` in megabytes (10^6) with one decimal, as the consent notice shows it.
pub fn megabytes(bytes: u64) -> String {
    let tenths = bytes.saturating_add(50_000) / 100_000;
    format!("{}.{}", tenths / 10, tenths % 10)
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
