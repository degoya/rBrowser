//! The repositories the catalogue reads and why one of its steps failed (RB-091-01).
//!
//! Adapted from rDownloader's `crates/rd-plugin-host/src/repository_types.rs` (RD-140-01): a
//! repository is the official catalogue - its address and key built in - or one a person added
//! in Settings > Plugins > Catalog with its https address and its public key. **Only a person
//! adds one**: the setting `plugins.catalog.repositories` is protected from MCP, like every key
//! that widens what reaches the network or what may be installed.

use ed25519_dalek::VerifyingKey;
use url::Url;

use crate::index::{IndexError, IndexSigners};
use crate::sign::{decode_public_key, hex_sha256, key_fingerprint};

/// The id of the official catalogue.
pub const OFFICIAL_REPOSITORY: &str = "official";
/// Where the official index is published: the asset of the public repository's fixed release
/// `plugin-index` (`.github/workflows/release-plugins.yml`), so the address never changes. Not
/// `releases/latest/download/`: that is the self-update's address, which a build without the
/// feature must not carry (`scripts/check-update-code.sh`).
pub const OFFICIAL_INDEX_URL: &str =
    "https://github.com/degoya/rBrowser/releases/download/plugin-index/rbrowser-plugin-index.json";
/// Most repositories a person may add.
pub const MAX_USER_REPOSITORIES: usize = 16;
/// Longest repository address.
const MAX_URL_BYTES: usize = 1024;
/// Longest repository name.
const MAX_NAME_CHARS: usize = 80;

/// One repository the catalogue reads.
#[derive(Clone, Debug)]
pub struct Repository {
    /// `official`, or `user-<16 hex>` from the address and the key: stable while both are, and
    /// safe as part of a file name.
    pub id: String,
    /// What the interface calls it; the official one's is translated by the interface.
    pub name: String,
    /// The index's address, https only.
    pub url: Url,
    /// The key(s) its index must be signed with.
    pub signers: IndexSigners,
}

impl Repository {
    /// The official catalogue: its built-in address, signed with the plugin release key the app
    /// also verifies the first-party packages with.
    ///
    /// # Errors
    ///
    /// A compiled-in root does not decode.
    pub fn official() -> Result<Self, RepositoryError> {
        Self::official_with(
            OFFICIAL_INDEX_URL,
            IndexSigners::official().map_err(|_| RepositoryError::InvalidKey)?,
        )
    }

    /// The official catalogue at another address or under other roots: for the tests, which
    /// sign with a key of their own.
    ///
    /// # Errors
    ///
    /// The address is not https.
    pub fn official_with(url: &str, signers: IndexSigners) -> Result<Self, RepositoryError> {
        Ok(Self {
            id: OFFICIAL_REPOSITORY.to_owned(),
            name: "rBrowser".to_owned(),
            url: parse_https(url)?,
            signers,
        })
    }

    /// A repository a person added, by its https address and its base64 Ed25519 public key.
    ///
    /// # Errors
    ///
    /// [`RepositoryError::InvalidUrl`] or [`RepositoryError::InvalidKey`].
    pub fn user(name: &str, url: &str, public_key: &str) -> Result<Self, RepositoryError> {
        let url = parse_https(url)?;
        let key = decode_public_key(public_key).map_err(|_| RepositoryError::InvalidKey)?;
        let name = name.trim();
        let name = if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
            url.host_str().unwrap_or_default().to_owned()
        } else {
            name.chars().filter(|c| !c.is_control()).collect()
        };
        Ok(Self {
            id: user_repository_id(&url, &key),
            name,
            url,
            signers: IndexSigners::Pinned(key),
        })
    }

    /// Whether this is the official catalogue.
    #[must_use]
    pub fn is_official(&self) -> bool {
        self.id == OFFICIAL_REPOSITORY
    }

    /// The fingerprint of a pinned key, for the interface; `None` for the official catalogue.
    #[must_use]
    pub fn key_fingerprint(&self) -> Option<String> {
        match &self.signers {
            IndexSigners::Pinned(key) => Some(key_fingerprint(key)),
            IndexSigners::Roots(_) => None,
        }
    }
}

/// `user-` and the first 16 hex characters of SHA-256 over the address and the key.
fn user_repository_id(url: &Url, key: &VerifyingKey) -> String {
    let mut material = url.as_str().as_bytes().to_vec();
    material.push(0);
    material.extend_from_slice(key.as_bytes());
    let digest = hex_sha256(&material);
    format!("user-{}", &digest[..16])
}

/// An absolute https URL with a host, without credentials or a fragment, at most 1,024 bytes.
///
/// # Errors
///
/// [`RepositoryError::InvalidUrl`].
pub fn parse_https(value: &str) -> Result<Url, RepositoryError> {
    let value = value.trim();
    if value.len() > MAX_URL_BYTES {
        return Err(RepositoryError::InvalidUrl);
    }
    let url = Url::parse(value).map_err(|_| RepositoryError::InvalidUrl)?;
    if url.scheme() != "https"
        || url.host_str().is_none_or(str::is_empty)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(RepositoryError::InvalidUrl);
    }
    Ok(url)
}

/// Why a catalogue step failed; [`code`](Self::code) is an i18n key the interface translates.
/// No variant carries a URL's query or a key: the texts reach logs and the status bar.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("the repository address is not an https URL")]
    InvalidUrl,
    #[error("the repository key is no Ed25519 public key")]
    InvalidKey,
    #[error("the download failed: {0}")]
    Download(String),
    #[error("the answer is larger than {limit} bytes")]
    TooLarge { limit: u64 },
    #[error(transparent)]
    Index(#[from] IndexError),
    #[error("the catalogue does not offer {id} {version} from that repository")]
    NotOffered { id: String, version: String },
    #[error("the plugin does not run on this rBrowser")]
    Incompatible,
    #[error("the package is {got} bytes, the index says {expected}")]
    SizeMismatch { expected: u64, got: u64 },
    #[error("the package's SHA-256 is not the one the index names")]
    ChecksumMismatch,
    /// The package verifier refused it; `code` is its refusal code (`plugins.error.*`).
    #[error("the package is refused: {reason}")]
    Package { code: &'static str, reason: String },
    #[error("the package is unsigned; the catalogue installs signed packages only")]
    Unsigned,
    #[error("the package is not what the index describes: {0}")]
    NotAsDescribed(String),
    #[error("the package asks for other capabilities than the ones confirmed")]
    ConsentMismatch,
    /// The package's key is not trusted here: the person confirms it in a step of its own, which
    /// names that the key is trusted for every plugin it signs.
    #[error("the package is signed by key {key_id} ({fingerprint}), which is not trusted")]
    KeyUntrusted { key_id: String, fingerprint: String },
    /// An update signed by another key than the installed version: confirmed separately.
    #[error("the update is signed by another key ({fingerprint}) than the installed version")]
    KeyChanged {
        fingerprint: String,
        previous: String,
    },
    /// A plugin file of this id is in the user folder and the catalogue did not put it there.
    #[error("a plugin {0} the catalogue did not install is in the plugin folder")]
    WouldOverwrite(String),
    /// The catalogue installed this plugin at the same or a higher version.
    #[error("{installed} is installed; {offered} is not newer")]
    NotNewer { installed: String, offered: String },
    #[error("the plugin was not installed from the catalogue")]
    NotInstalledHere,
    #[error("{0}")]
    Io(String),
}

impl RepositoryError {
    /// The stable code, an i18n key of `locales/en.json`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidUrl => "plugins.catalog.error.invalid_url",
            Self::InvalidKey => "plugins.catalog.error.invalid_key",
            Self::Download(_) => "plugins.catalog.error.download",
            Self::TooLarge { .. } => "plugins.catalog.error.too_large",
            Self::Index(error) => error.code(),
            Self::NotOffered { .. } => "plugins.catalog.error.not_offered",
            Self::Incompatible => "plugins.catalog.error.incompatible",
            Self::SizeMismatch { .. } => "plugins.catalog.error.size_mismatch",
            Self::ChecksumMismatch => "plugins.catalog.error.checksum",
            Self::Package { code, .. } => code,
            Self::Unsigned => "plugins.catalog.error.unsigned",
            Self::NotAsDescribed(_) => "plugins.catalog.error.not_as_described",
            Self::ConsentMismatch => "plugins.catalog.error.consent_mismatch",
            Self::KeyUntrusted { .. } => "plugins.catalog.error.key_untrusted",
            Self::KeyChanged { .. } => "plugins.catalog.error.key_changed",
            Self::WouldOverwrite(_) => "plugins.catalog.error.would_overwrite",
            Self::NotNewer { .. } => "plugins.catalog.error.not_newer",
            Self::NotInstalledHere => "plugins.catalog.error.not_installed_here",
            Self::Io(_) => "plugins.catalog.error.io",
        }
    }
}

impl From<std::io::Error> for RepositoryError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}
