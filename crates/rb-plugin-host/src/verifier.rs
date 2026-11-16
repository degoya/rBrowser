//! `PluginVerifier`: the trust store and the checks a package passes before anything runs it -
//! the archive with bounded members, the manifest, the members it names, the locales, the
//! settings schema, the signature against the content digest, and the compile with its import
//! and world check.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host/src/verifier.rs` (itself from
//! rDownloader's): the same order - the signature decides before a byte of the component is
//! parsed - a trusted `key_id` pinned to its key, an unknown key a trust decision rather than a
//! refusal ([`VerifyError::UntrustedKey`], trust on first use). Not taken (yet): rCommander's
//! revoked digests and keys and the installed-key check, which come with the install flow of
//! Settings > Plugins (RB-010-08).

use std::{
    io::{Cursor, Read, Seek},
    path::Path,
};

use anyhow::{Context, Result};
use ed25519_dalek::VerifyingKey;
use rb_plugin_types::codes;

use crate::{
    contents::{check_locale, check_settings_schema},
    manifest::{PluginManifest, parse_manifest},
    package::{
        MAX_PACKAGE_BYTES, PackageParts, VerifiedPackage, check_members, package_digest,
        read_archive,
    },
    plugin::{LoadedPlugin, PluginOptions},
    refusal::{refusal_code, refuse},
    sign::{TrustStore, decode_public_key, key_fingerprint, verify_detached_bytes},
};

/// The rBrowser version a plugin's `metadata.min_app_version` is compared with.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Why a package was refused, keeping the trust decision a person can make apart from the
/// refusals nobody can override.
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    /// Consistent and signed by the key its manifest names, which this installation does not
    /// trust yet: the install preview shows the fingerprint and asks (trust on first use).
    #[error("untrusted signing key {key_id} (fingerprint {fingerprint})")]
    UntrustedKey {
        key_id: String,
        /// The base64 key the package declares, pinned once a person confirms it.
        public_key: String,
        fingerprint: String,
        id: String,
        version: String,
    },
    /// Anything else: a broken or changed package, an import the manifest does not grant.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl VerifyError {
    /// The stable code: `plugins.error.key_untrusted` for the trust decision, the refusal's for
    /// every other.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::UntrustedKey { .. } => codes::KEY_UNTRUSTED,
            Self::Other(error) => refusal_code(error),
        }
    }
}

/// The trust store and the policy packages are verified under. Clones share the keys.
#[derive(Clone, Debug, Default)]
pub struct PluginVerifier {
    trusted_keys: TrustStore,
    development_mode: bool,
}

impl PluginVerifier {
    /// A verifier trusting nothing yet; unsigned packages only in development mode.
    #[must_use]
    pub fn new(development_mode: bool) -> Self {
        Self {
            trusted_keys: TrustStore::new(),
            development_mode,
        }
    }

    /// A verifier trusting the compiled-in roots (`sign::EMBEDDED_KEYS`).
    ///
    /// # Errors
    ///
    /// A root does not decode.
    pub fn with_roots(development_mode: bool) -> Result<Self> {
        Ok(Self {
            trusted_keys: TrustStore::with_roots()?,
            development_mode,
        })
    }

    #[must_use]
    pub fn development_mode(&self) -> bool {
        self.development_mode
    }

    /// Trusts `key` under `key_id`: a root, or a key a person confirmed on first use (pinned in
    /// the protected setting `plugins.trustedKeys`). Never on an MCP caller's word.
    ///
    /// # Errors
    ///
    /// The key id is empty.
    pub fn trust_key(&self, key_id: String, key: VerifyingKey) -> Result<()> {
        self.trusted_keys
            .trust(key_id, key)
            .context("trust a plugin signing key")
    }

    /// [`PluginVerifier::trust_key`] from the key's base64 form.
    ///
    /// # Errors
    ///
    /// The key does not decode.
    pub fn trust_key_base64(&self, key_id: String, encoded: &str) -> Result<()> {
        self.trust_key(key_id, decode_public_key(encoded)?)
    }

    /// Reads and verifies one package file.
    ///
    /// # Errors
    ///
    /// The file cannot be read or is larger than any package; see
    /// [`PluginVerifier::verify_bytes`].
    pub fn verify_file(&self, path: &Path) -> Result<VerifiedPackage, VerifyError> {
        let size = std::fs::metadata(path)
            .with_context(|| format!("open plugin package {}", path.display()))?
            .len();
        if size > MAX_PACKAGE_BYTES {
            return Err(refuse(codes::INVALID, "the package is larger than any plugin").into());
        }
        let file = std::fs::File::open(path)
            .with_context(|| format!("open plugin package {}", path.display()))?;
        self.verify_reader(file)
    }

    /// Verifies a package in memory.
    ///
    /// # Errors
    ///
    /// [`VerifyError::UntrustedKey`] for a consistent package by a key nobody confirmed; a
    /// refusal with its code in [`VerifyError::Other`] for everything else.
    pub fn verify_bytes(&self, bytes: &[u8]) -> Result<VerifiedPackage, VerifyError> {
        self.verify_reader(Cursor::new(bytes))
    }

    fn verify_reader<R: Read + Seek>(&self, reader: R) -> Result<VerifiedPackage, VerifyError> {
        let parts =
            read_archive(reader).map_err(|error| refuse(codes::INVALID, format!("{error:#}")))?;
        let package = self.verify_contents(parts)?;
        check_component(&package)?;
        Ok(package)
    }

    /// Everything the signature decides, without parsing the component.
    pub(crate) fn verify_contents(
        &self,
        parts: PackageParts,
    ) -> Result<VerifiedPackage, VerifyError> {
        let manifest = parse_manifest(&parts.manifest).map_err(|error| {
            let code = refusal_code(&error);
            refuse(code, format!("{error:#}"))
        })?;
        check_app_version(&manifest)?;
        check_members(&manifest, &parts.files)
            .map_err(|error| refuse(codes::INVALID, format!("{error:#}")))?;
        let digest = package_digest(&parts.manifest, &parts.component, &parts.files);
        // The signature before anything else of the content is interpreted.
        match parts.signature.as_deref() {
            Some(encoded) => self.verify_signed(&manifest, &digest, encoded)?,
            None if !self.development_mode => {
                return Err(refuse(
                    codes::UNSIGNED,
                    "the package is unsigned; unsigned plugins need development mode",
                )
                .into());
            }
            None => tracing::warn!(
                plugin_id = %manifest.id,
                version = %manifest.version,
                "an unsigned plugin package is accepted in development mode"
            ),
        }
        let package = VerifiedPackage {
            manifest,
            manifest_bytes: parts.manifest,
            component: parts.component,
            signature: parts.signature,
            files: parts.files,
        };
        for (language, bytes) in package.locales() {
            check_locale(&package.manifest.id, bytes).map_err(|error| {
                refuse(
                    codes::INVALID,
                    format!("locales/{language}.json: {error:#}"),
                )
            })?;
        }
        if let Some(schema) = package.settings_schema() {
            check_settings_schema(schema).map_err(|error| {
                let code = refusal_code(&error);
                refuse(code, format!("{error:#}"))
            })?;
        }
        Ok(package)
    }

    /// The signature against the manifest's key: a trusted `key_id` pinned to its key, an
    /// unknown one checked for consistency and handed to a person.
    fn verify_signed(
        &self,
        manifest: &PluginManifest,
        digest: &[u8; 32],
        encoded: &[u8],
    ) -> Result<(), VerifyError> {
        let declared = decode_public_key(&manifest.public_key)
            .map_err(|error| refuse(codes::INVALID, format!("public_key: {error:#}")))?;
        let checked = |key: &VerifyingKey| {
            verify_detached_bytes(key, digest, encoded).map_err(|error| {
                refuse(
                    codes::SIGNATURE_INVALID,
                    format!(
                        "the signature of {} {} does not verify: the package was changed after \
                         it was signed ({error:#})",
                        manifest.id, manifest.version
                    ),
                )
            })
        };
        match self.trusted_keys.key(&manifest.key_id)? {
            Some(trusted) if trusted == declared => Ok(checked(&trusted)?),
            Some(_) => Err(refuse(
                codes::KEY_MISMATCH,
                format!(
                    "the package's public_key is not the key trusted as {}",
                    manifest.key_id
                ),
            )
            .into()),
            None => {
                checked(&declared)?;
                Err(VerifyError::UntrustedKey {
                    key_id: manifest.key_id.clone(),
                    public_key: manifest.public_key.clone(),
                    fingerprint: key_fingerprint(&declared),
                    id: manifest.id.clone(),
                    version: manifest.version.to_string(),
                })
            }
        }
    }
}

/// `plugins.error.needs_newer_app` when the plugin asks for a newer rBrowser.
fn check_app_version(manifest: &PluginManifest) -> Result<()> {
    let Some(min) = &manifest.metadata.min_app_version else {
        return Ok(());
    };
    let min = semver::Version::parse(min).context("metadata.min_app_version")?;
    let app = semver::Version::parse(APP_VERSION).context("the app version")?;
    if app < min {
        return Err(refuse(
            codes::NEEDS_NEWER_APP,
            format!("{} needs rBrowser {min} or later", manifest.id),
        ));
    }
    Ok(())
}

/// Compiles the component once, refusing an import its manifest does not grant or a declared
/// kind it does not export: what loading it would refuse, said at install.
pub(crate) fn check_component(package: &VerifiedPackage) -> Result<()> {
    LoadedPlugin::load(package, PluginOptions::default())
        .map(drop)
        .map_err(|failure| {
            let code = match failure.code() {
                Some(code @ (codes::CAPABILITY_DENIED | codes::DENIED | codes::WORLD_MISSING)) => {
                    codes::ALL
                        .iter()
                        .copied()
                        .find(|known| *known == code)
                        .unwrap_or(codes::INVALID)
                }
                _ => codes::INVALID,
            };
            let detail: Vec<String> = failure
                .params
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect();
            refuse(
                code,
                format!(
                    "the plugin component is refused: {code} ({})",
                    detail.join(", ")
                ),
            )
        })
}

#[cfg(test)]
#[path = "verifier_tests.rs"]
mod tests;
