//! One Web Store install end to end (RB-080-23): the download ([`super::webstore`]), the CRX3
//! check ([`super::crx3`]), the manifest read out of the ZIP for the question in the window,
//! and the unpacking into the store ([`super::store`]). A [`Candidate`] is a downloaded and
//! verified CRX that is not unpacked yet - what the prompt names before the person decides.
//! Blocking: the callers run it on a blocking thread.

use super::crx3::{self, Verified, verify};
use super::store::{ExtensionStore, InstallError, InstalledExtension, peek_manifest};
use super::update_check::check_version;
use super::webstore::{DownloadError, HopFetcher, MAX_CRX_BYTES, download};

/// A downloaded and verified CRX, not unpacked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The extension id.
    pub id: String,
    /// The manifest's name.
    pub name: String,
    /// The manifest's version.
    pub version: String,
    /// The CRX; verified again before it is unpacked.
    bytes: Vec<u8>,
}

/// Why an install failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InstallFailure {
    /// The download.
    #[error(transparent)]
    Download(#[from] DownloadError),
    /// The CRX check.
    #[error(transparent)]
    Crx(#[from] crx3::Crx3Error),
    /// The unpacking.
    #[error(transparent)]
    Install(#[from] InstallError),
}

impl InstallFailure {
    /// The stable code of the error (MCP answers, notices).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Download(error) => error.code(),
            Self::Crx(error) => error.code(),
            Self::Install(error) => error.code(),
        }
    }
}

/// The pieces of an install.
pub struct Installer<'a> {
    /// The network.
    pub fetcher: &'a dyn HopFetcher,
    /// The store on disk.
    pub store: &'a ExtensionStore,
    /// The `prodversion` sent to the update server.
    pub chromium_version: String,
    /// The publisher key the CRX must carry a proof of.
    pub publisher_key_hash: [u8; 32],
}

impl Installer<'_> {
    /// Downloads and verifies the CRX of `id`; nothing is unpacked.
    ///
    /// # Errors
    ///
    /// The download or the check.
    pub fn fetch(&self, id: &str) -> Result<Candidate, InstallFailure> {
        let bytes = download(self.fetcher, id, &self.chromium_version, MAX_CRX_BYTES)?;
        let verified = verify(&bytes, id, &self.publisher_key_hash)?;
        let manifest = peek_manifest(verified.zip)?;
        tracing::info!(id, version = %manifest.version, "a Web Store extension was downloaded and verified");
        Ok(Candidate {
            id: verified.id,
            name: manifest.name,
            version: manifest.version,
            bytes,
        })
    }

    /// Unpacks `candidate` into the store, verified once more.
    ///
    /// # Errors
    ///
    /// The check or the unpacking; nothing new is active then.
    pub fn install(&self, candidate: &Candidate) -> Result<InstalledExtension, InstallFailure> {
        let verified: Verified<'_> =
            verify(&candidate.bytes, &candidate.id, &self.publisher_key_hash)?;
        let installed = self.store.install(&verified)?;
        tracing::info!(id = %installed.id, version = %installed.version, "a Web Store extension is installed");
        Ok(installed)
    }

    /// [`Installer::fetch`] then [`Installer::install`]: the person's own command.
    ///
    /// # Errors
    ///
    /// Any step's.
    pub fn install_by_id(&self, id: &str) -> Result<InstalledExtension, InstallFailure> {
        let candidate = self.fetch(id)?;
        self.install(&candidate)
    }

    /// The store's version of `installed` when it is newer; `None` when it is not. The update
    /// check names the version first ([`check_version`]); the CRX is downloaded only when that
    /// is newer, or when the check names nothing (the download decides then).
    ///
    /// # Errors
    ///
    /// The download or the check.
    pub fn newer(
        &self,
        installed: &InstalledExtension,
    ) -> Result<Option<Candidate>, InstallFailure> {
        match check_version(self.fetcher, &installed.id, &self.chromium_version) {
            Ok(Some(version)) if !is_newer(&version, &installed.version) => {
                tracing::debug!(id = %installed.id, %version, "the Web Store has no newer version");
                return Ok(None);
            }
            Ok(_) => {}
            Err(error) => {
                tracing::debug!(id = %installed.id, "the update check failed, the download decides: {error}");
            }
        }
        let candidate = self.fetch(&installed.id)?;
        Ok(is_newer(&candidate.version, &installed.version).then_some(candidate))
    }
}

/// Whether `candidate` is a newer version than `installed`: number by number, a missing part
/// counts as 0 (Chromium's rule).
pub fn is_newer(candidate: &str, installed: &str) -> bool {
    let parts = |version: &str| -> Option<Vec<u32>> {
        version.split('.').map(|part| part.parse().ok()).collect()
    };
    let (Some(new), Some(old)) = (parts(candidate), parts(installed)) else {
        return false;
    };
    let len = new.len().max(old.len());
    let at = |list: &[u32], index: usize| list.get(index).copied().unwrap_or(0);
    (0..len)
        .map(|index| at(&new, index).cmp(&at(&old, index)))
        .find(|order| order.is_ne())
        .is_some_and(|order| order.is_gt())
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
