//! The plugin catalogue (RB-091-01): signed repositories, their verified indexes, the offers and
//! updates read from them, and the install of an offered package.
//!
//! Adapted from rDownloader's `crates/rd-plugin-host/src/repository.rs` (RD-140-01), without its
//! database and services: the app builds a [`Catalog`] from the settings, refreshes it off the UI
//! thread and answers the catalogue page from it. An index is fetched over https, verified against
//! its repository's key and replay floor (the higher of the stored floor and the cached index's
//! sequence), the floor is raised *before* the index is cached, and at start every cached index
//! is verified again - signature, floor, expiry - and raises the floor when it holds, or is
//! deleted rather than trusted, so an installation without network offers only what still
//! verifies.
//!
//! **A repository delivers; it vouches for nothing.** A package offered here is downloaded within
//! the size its index names, held to its SHA-256, then verified as a `.rbplugin` file is - by
//! [`crate::PluginVerifier`], signed, under a trusted or person-confirmed key - and installed into
//! the user's plugin folder only with the capabilities the person confirmed ([`install`]).
//! **Never on an agent's word**: nothing here is reachable from MCP; the app's catalogue commands
//! that install or add a repository are `omitted` with `PROTECTED_REASON`.

use std::collections::BTreeMap;
use std::time::Duration;

use time::OffsetDateTime;
use url::Url;

use crate::index::{
    self, CatalogEntry, IndexEntry, MAX_INDEX_BYTES, Offer, PluginIndex, StaleError, merge_offers,
    update_for,
};
use crate::verifier::APP_VERSION;

#[path = "repository_fetch.rs"]
mod fetch;
#[path = "repository_install.rs"]
mod install;
#[path = "repository_prepare.rs"]
mod prepare;
#[path = "repository_select.rs"]
mod select;
#[path = "repository_store.rs"]
mod store;
#[path = "repository_types.rs"]
mod types;

#[cfg(any(test, feature = "test-support"))]
pub use fetch::MemoryFetcher;
pub use fetch::{
    CONNECT_TIMEOUT_SECONDS, Fetcher, HttpFetcher, INDEX_TIMEOUT_SECONDS, MAX_REDIRECTS,
    PACKAGE_TIMEOUT_SECONDS, user_agent,
};
pub use install::{Consent, Installed, PinnedKey, download, install, remove};
pub use prepare::{KeyReview, Prepared, prepare};
pub use select::{Reachability, Shipped, UNREACHABLE_FOR, newer_than, replacement};
pub(crate) use store::write_atomic;
pub use store::{CatalogStore, FLOORS_FILE, INSTALLS_FILE, InstallRecord};
pub use types::{
    MAX_USER_REPOSITORIES, OFFICIAL_INDEX_URL, OFFICIAL_REPOSITORY, Repository, RepositoryError,
    parse_https,
};

/// The folder of the catalogue's files beside the user settings.
pub const CATALOG_DIR: &str = "plugin-catalog";

/// One repository's verified index and the address it came from (relative package URLs resolve
/// against it).
#[derive(Clone, Debug)]
pub struct LoadedIndex {
    pub url: Url,
    pub index: PluginIndex,
}

/// The repositories, their verified indexes and the catalogue's store.
#[derive(Debug)]
pub struct Catalog {
    store: CatalogStore,
    repositories: Vec<Repository>,
    loaded: BTreeMap<String, LoadedIndex>,
}

impl Catalog {
    /// A catalogue of `repositories` (the official one first, then the person's in their order)
    /// over `store`, with nothing loaded yet.
    #[must_use]
    pub fn new(store: CatalogStore, repositories: Vec<Repository>) -> Self {
        Self {
            store,
            repositories,
            loaded: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn repositories(&self) -> &[Repository] {
        &self.repositories
    }

    #[must_use]
    pub fn store(&self) -> &CatalogStore {
        &self.store
    }

    /// Loads every cached index that still verifies at `now`; one that does not is deleted. The
    /// offline rule: without a refresh, only what still verifies is offered.
    pub fn load_cached(&mut self, now: OffsetDateTime) {
        let floors = self.store.floors();
        for repository in &self.repositories {
            let Some(bytes) = self.store.cached_index(&repository.id) else {
                continue;
            };
            // No known sequence: the cache *is* the floor, and passing it in would make the cache
            // a replay of itself. It may not sit below the floor, though.
            let verified = index::verify(&bytes, &repository.signers, None, now)
                .map_err(RepositoryError::from)
                .and_then(|index| at_floor(index, floors.get(&repository.id).copied()));
            match verified {
                Ok(index) => {
                    // A verified cache is a floor too (security review M2): a floors file lost
                    // or reset never lets an older index in over it.
                    if let Err(error) = self.store.raise_floor(&repository.id, index.sequence) {
                        tracing::warn!(
                            repository = %repository.id,
                            code = error.code(),
                            "the replay floor of a cached plugin index was not raised"
                        );
                    }
                    self.loaded.insert(
                        repository.id.clone(),
                        LoadedIndex {
                            url: repository.url.clone(),
                            index,
                        },
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        repository = %repository.id,
                        code = error.code(),
                        "a cached plugin index does not verify; it is deleted"
                    );
                    self.store.drop_index(&repository.id);
                }
            }
        }
    }

    /// Fetches and verifies every repository's index, one after the other; a failure of one is
    /// returned with its id and does not stop the others, and keeps what it had loaded.
    pub fn refresh(
        &mut self,
        fetcher: &dyn Fetcher,
        now: OffsetDateTime,
    ) -> Vec<(String, Result<(), RepositoryError>)> {
        let repositories = self.repositories.clone();
        repositories
            .iter()
            .map(|repository| {
                let outcome = self.refresh_one(fetcher, repository, now);
                if let Err(error) = &outcome {
                    tracing::warn!(
                        repository = %repository.id,
                        code = error.code(),
                        "a plugin repository refresh failed"
                    );
                }
                (repository.id.clone(), outcome)
            })
            .collect()
    }

    fn refresh_one(
        &mut self,
        fetcher: &dyn Fetcher,
        repository: &Repository,
        now: OffsetDateTime,
    ) -> Result<(), RepositoryError> {
        let limit = u64::try_from(MAX_INDEX_BYTES).unwrap_or(u64::MAX);
        let timeout = Duration::from_secs(INDEX_TIMEOUT_SECONDS);
        let bytes = fetcher.fetch(&repository.url, limit, timeout)?;
        // The floor is the higher of the stored one and the sequence of the index loaded from the
        // cache (security review M2).
        let loaded = self
            .loaded
            .get(&repository.id)
            .map(|loaded| loaded.index.sequence);
        let floor = self.store.floors().get(&repository.id).copied().max(loaded);
        // The same bytes again is the ordinary case - an index changes with a release, not with
        // every refresh - and is not a replay: only its own floor is held to it, as at start.
        let unchanged = self.store.cached_index(&repository.id).as_deref() == Some(&bytes[..]);
        let known = if unchanged { None } else { floor };
        let index = index::verify(&bytes, &repository.signers, known, now)?;
        let index = at_floor(index, floor)?;
        // The floor first: a crash after it leaves the old cache refused at the next start, never
        // an older index accepted over a newer one.
        self.store.raise_floor(&repository.id, index.sequence)?;
        self.store.cache_index(&repository.id, &bytes)?;
        self.loaded.insert(
            repository.id.clone(),
            LoadedIndex {
                url: repository.url.clone(),
                index,
            },
        );
        Ok(())
    }

    /// The indexes loaded and not expired at `now`, in the repositories' order.
    fn current(&self, now: OffsetDateTime) -> Vec<(&Repository, &LoadedIndex)> {
        self.repositories
            .iter()
            .filter_map(|repository| {
                self.loaded
                    .get(&repository.id)
                    .filter(|loaded| loaded.index.expires_at >= now)
                    .map(|loaded| (repository, loaded))
            })
            .collect()
    }

    /// Whether an index of `repository` is loaded and current at `now`.
    #[must_use]
    pub fn is_loaded(&self, repository: &str, now: OffsetDateTime) -> bool {
        self.current(now)
            .iter()
            .any(|(loaded, _)| loaded.id == repository)
    }

    /// Every plugin offered at `now`, each with every repository's offer.
    #[must_use]
    pub fn entries(&self, now: OffsetDateTime) -> Vec<CatalogEntry> {
        let indexes: Vec<(String, &PluginIndex)> = self
            .current(now)
            .into_iter()
            .map(|(repository, loaded)| (repository.id.clone(), &loaded.index))
            .collect();
        merge_offers(&indexes, &app_version())
    }

    /// The entry `repository` offers as `id` at `version`, with the address its package
    /// resolves against.
    ///
    /// # Errors
    ///
    /// [`RepositoryError::NotOffered`].
    pub fn offer(
        &self,
        repository: &str,
        id: &str,
        version: &str,
        now: OffsetDateTime,
    ) -> Result<(Url, IndexEntry), RepositoryError> {
        self.current(now)
            .into_iter()
            .find(|(candidate, _)| candidate.id == repository)
            .and_then(|(_, loaded)| {
                loaded
                    .index
                    .plugins
                    .iter()
                    .find(|entry| entry.id == id && entry.version == version)
                    .map(|entry| (loaded.url.clone(), entry.clone()))
            })
            .ok_or_else(|| RepositoryError::NotOffered {
                id: id.to_owned(),
                version: version.to_owned(),
            })
    }

    /// The updates of the plugins the catalogue installed: each from the repository it came from.
    #[must_use]
    pub fn updates(&self, now: OffsetDateTime) -> Vec<Offer> {
        let entries = self.entries(now);
        self.store
            .installs()
            .iter()
            .filter_map(|(id, record)| {
                let installed = semver::Version::parse(&record.version).ok()?;
                let entry = entries.iter().find(|entry| &entry.id == id)?;
                update_for(entry, &record.repository, &installed).cloned()
            })
            .collect()
    }
}

/// The app's version, as the compatibility of an entry is judged.
fn app_version() -> semver::Version {
    semver::Version::parse(APP_VERSION).unwrap_or_else(|_| semver::Version::new(0, 0, 0))
}

/// `index` unless its sequence is below `floor`.
fn at_floor(index: PluginIndex, floor: Option<u64>) -> Result<PluginIndex, RepositoryError> {
    match floor {
        Some(known) if index.sequence < known => Err(RepositoryError::Index(
            StaleError::Replayed {
                saw: index.sequence,
                known,
            }
            .into(),
        )),
        _ => Ok(index),
    }
}

#[cfg(test)]
#[path = "repository_fixtures.rs"]
pub(crate) mod fixtures;
#[cfg(test)]
#[path = "repository_tests.rs"]
mod tests;
