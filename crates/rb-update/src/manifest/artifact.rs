//! One downloadable file of a release as the signed manifest lists it, and the rules every entry
//! obeys.
//!
//! Copied and adapted from rCommander's `crates/rc-update/src/manifest/artifact.rs` (RB-090-02):
//! the kind is rBrowser's portable ZIP (`scripts/package-windows.sh`, RB-090-01).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::sign::is_sha256;

/// One downloadable file. `platform`, `arch` and `kind` are open strings on purpose: a later
/// release that adds a platform (phase 2) must not make every older installation refuse the
/// manifest; what this build does not know it never selects.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Artifact {
    /// `windows` in phase 1; `linux` and `macos` follow.
    pub platform: String,
    /// `x86_64` in phase 1.
    pub arch: String,
    /// [`kind::ZIP`].
    pub kind: String,
    /// Absolute `https://` URL.
    pub url: String,
    /// 64 lowercase hex characters.
    pub sha256: String,
    /// Exact byte size; the download is cut off at it.
    pub size: u64,
}

/// The artifact kinds this build knows: the portable archive `scripts/package-windows.sh`
/// writes. The whole program folder is replaced (no delta): CEF moves with every release.
pub mod kind {
    /// `rbrowser-<v>-windows-x64.zip`.
    pub const ZIP: &str = "zip";
}

impl Artifact {
    fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("platform", &self.platform),
            ("arch", &self.arch),
            ("kind", &self.kind),
        ] {
            if value.is_empty()
                || value.len() > 32
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            {
                return Err(format!("{name} {value:?} is not a lowercase identifier"));
            }
        }
        if self.url.len() > 1024 {
            return Err("the URL is longer than 1024 bytes".to_owned());
        }
        if !is_plain_https(&self.url) {
            return Err(format!("{} is not a plain https:// URL", self.url));
        }
        if !is_sha256(&self.sha256) {
            return Err("sha256 is not 64 lowercase hex characters".to_owned());
        }
        if self.size == 0 || self.size > super::MAX_ARTIFACT_BYTES {
            return Err(format!(
                "size {} is outside 1..={}",
                self.size,
                super::MAX_ARTIFACT_BYTES
            ));
        }
        Ok(())
    }
}

/// An absolute `https://` URL with a host and without a login.
#[must_use]
pub fn is_plain_https(text: &str) -> bool {
    url::Url::parse(text).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some_and(|host| !host.is_empty())
            && url.username().is_empty()
            && url.password().is_none()
    })
}

/// Refuses a list with an entry that breaks the rules above, or with one platform, architecture
/// and kind listed twice.
pub(super) fn validate_list(artifacts: &[Artifact]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for artifact in artifacts {
        let entry = format!("{}/{}/{}", artifact.platform, artifact.arch, artifact.kind);
        artifact
            .validate()
            .map_err(|reason| format!("{entry}: {reason}"))?;
        if !seen.insert((
            artifact.platform.as_str(),
            artifact.arch.as_str(),
            artifact.kind.as_str(),
        )) {
            return Err(format!("{entry} is listed twice"));
        }
    }
    Ok(())
}
