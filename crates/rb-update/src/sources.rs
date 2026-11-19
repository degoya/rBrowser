//! Where the manifests are published, and one check of every manifest a channel reads
//! (RB-090-14).
//!
//! **Where.** The stable manifest is an asset of every plain `vX.Y.Z` release, so
//! `releases/latest/download/rbrowser-update-stable.json` always names the newest one - GitHub's
//! `latest` never points at a pre-release. A beta has no fixed address of that kind, so the beta
//! channel reads the release list (`api.github.com/repos/<repo>/releases`), takes the release of
//! the highest version that carries `rbrowser-update-beta.json` and fetches that asset. The list
//! is unsigned and only *locates* the manifest: the asset must lie under the repository's own
//! `releases/download/`, and what is fetched verifies under the compiled-in root or is refused.
//! The beta channel reads the stable manifest too, so `0.9.0-beta.3` is offered `0.9.0`.
//!
//! **The pin.** Every artifact a verified manifest lists must lie under the same
//! `releases/download/` ([`pinned`]): even a signed manifest cannot send an installation
//! elsewhere.
//!
//! **Floors.** Each channel keeps its own (`floors.rs`); they are stored before any manifest is
//! handed on, and one that cannot be stored hands on none.
//!
//! Adapted from rCommander's `crates/rc-update/src/check.rs`: blocking, rBrowser's repository, no
//! mirror.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::error::UpdateError;
use crate::fetch::{BodyTooLarge, Fetcher, Url};
use crate::floors::Floors;
use crate::gate::Gate;
use crate::manifest::{self, Channel, MAX_MANIFEST_BYTES, UpdateManifest, parse_version};
use crate::sign::TrustStore;

/// The repository the official releases are published in (RB-090-12).
pub const OFFICIAL_REPOSITORY: &str = "degoya/rBrowser";
/// Releases the beta channel reads per list.
pub const RELEASE_LIST_PAGE: u32 = 30;
/// Largest release list read.
pub const MAX_RELEASE_LIST_BYTES: u64 = 8 * 1024 * 1024;

/// Where one source publishes its manifests.
#[derive(Clone, Debug)]
pub struct Sources {
    /// The stable manifest.
    pub stable: Url,
    /// The release list the beta manifest is looked up in.
    pub release_list: Url,
    /// Every asset address the list or a manifest may name starts with this.
    pub download_prefix: String,
}

impl Sources {
    /// The official repository's addresses.
    #[must_use]
    pub fn official() -> Self {
        let parse = |text: String| Url::parse(&text).expect("a constant address parses");
        Self {
            stable: parse(format!(
                "https://github.com/{OFFICIAL_REPOSITORY}/releases/latest/download/{}",
                Channel::Stable.file_name()
            )),
            release_list: parse(format!(
                "https://api.github.com/repos/{OFFICIAL_REPOSITORY}/releases?per_page={RELEASE_LIST_PAGE}"
            )),
            download_prefix: format!("https://github.com/{OFFICIAL_REPOSITORY}/releases/download/"),
        }
    }

    /// The page of the release `version` (`.../releases/tag/v<version>`): where an installation
    /// that does not update itself downloads it by hand.
    #[must_use]
    pub fn release_page(version: &str) -> String {
        format!("https://github.com/{OFFICIAL_REPOSITORY}/releases/tag/v{version}")
    }
}

/// What one check found.
#[derive(Debug, Default)]
pub struct CheckReport {
    /// Every manifest that verified, its floor stored.
    pub manifests: Vec<UpdateManifest>,
    /// What went wrong worth reporting: any refusal of a manifest's integrity, otherwise the
    /// first failure when nothing verified at all.
    pub problem: Option<UpdateError>,
}

/// Fetches and verifies the manifests `channel` reads, behind `gate` and against the
/// compiled-in root.
///
/// # Errors
///
/// `update.disabled` while the gate is shut (no request, no file), `update.not_configured` for a
/// build without a root.
pub fn check_channel(
    gate: Gate,
    fetcher: &dyn Fetcher,
    sources: &Sources,
    channel: Channel,
    data: &Path,
    now: DateTime<Utc>,
) -> Result<CheckReport, UpdateError> {
    if !gate.is_open() {
        return Err(UpdateError::Disabled);
    }
    let trust = manifest::release_trust(now)?;
    Ok(check_channel_with(
        &trust, fetcher, sources, channel, data, now,
    ))
}

/// [`check_channel`] behind an open gate, against `trust`.
#[must_use]
pub fn check_channel_with(
    trust: &TrustStore,
    fetcher: &dyn Fetcher,
    sources: &Sources,
    channel: Channel,
    data: &Path,
    now: DateTime<Utc>,
) -> CheckReport {
    let mut floors = Floors::load(data);
    let mut report = CheckReport::default();
    let mut failures = Vec::new();
    let stable = fetch_manifest(fetcher, &sources.stable);
    let verify = |fetched: Result<Vec<u8>, UpdateError>, channel: Channel, floors: &Floors| {
        fetched
            .and_then(|bytes| {
                manifest::verify_with(&bytes, trust, channel, floors.get(channel), now)
            })
            .and_then(|manifest| pinned(manifest, &sources.download_prefix))
    };
    let mut record = |verified: Result<UpdateManifest, UpdateError>,
                      channel: Channel,
                      floors: &mut Floors| match verified {
        Ok(manifest) => {
            floors.raise(channel, manifest.sequence);
            report.manifests.push(manifest);
        }
        Err(error) => {
            tracing::info!(channel = channel.as_str(), code = error.code(), %error, "an update manifest is not taken");
            failures.push(error);
        }
    };
    let verified = verify(stable, Channel::Stable, &floors);
    record(verified, Channel::Stable, &mut floors);
    if channel == Channel::Beta {
        let beta = locate_beta(fetcher, sources).and_then(|url| fetch_manifest(fetcher, &url));
        let verified = verify(beta, Channel::Beta, &floors);
        record(verified, Channel::Beta, &mut floors);
    }
    if !report.manifests.is_empty()
        && let Err(error) = floors.store(data)
    {
        // Nothing may act on a manifest whose floor is not stored.
        report.manifests.clear();
        failures.insert(0, UpdateError::Other(error));
    }
    let integrity = failures.iter().position(UpdateError::is_integrity);
    report.problem = match integrity {
        Some(index) => Some(failures.swap_remove(index)),
        None if report.manifests.is_empty() => failures.into_iter().next(),
        None => None,
    };
    report
}

/// Refuses a verified manifest with an artifact outside the source's own downloads, compared in
/// the parsed form the download requests, so `..` and its percent-encoded spelling leave nothing
/// out.
///
/// # Errors
///
/// `update.invalid` naming the artifact.
pub fn pinned(
    manifest: UpdateManifest,
    download_prefix: &str,
) -> Result<UpdateManifest, UpdateError> {
    let outside = manifest.artifacts.iter().find(|artifact| {
        !Url::parse(&artifact.url).is_ok_and(|url| url.as_str().starts_with(download_prefix))
    });
    match outside {
        Some(artifact) => Err(UpdateError::Invalid(format!(
            "the artifact {} is not under {download_prefix}",
            artifact.url
        ))),
        None => Ok(manifest),
    }
}

fn fetch_error(error: &anyhow::Error) -> UpdateError {
    if error.downcast_ref::<BodyTooLarge>().is_some() {
        UpdateError::TooLarge
    } else {
        UpdateError::Fetch(format!("{error:#}"))
    }
}

fn fetch_manifest(fetcher: &dyn Fetcher, url: &Url) -> Result<Vec<u8>, UpdateError> {
    let limit = u64::try_from(MAX_MANIFEST_BYTES).unwrap_or(u64::MAX);
    match fetcher.fetch(url, limit) {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => Err(UpdateError::NotPublished),
        Err(error) => Err(fetch_error(&error)),
    }
}

fn locate_beta(fetcher: &dyn Fetcher, sources: &Sources) -> Result<Url, UpdateError> {
    let list = match fetcher.fetch(&sources.release_list, MAX_RELEASE_LIST_BYTES) {
        Ok(Some(list)) => list,
        Ok(None) => return Err(UpdateError::NotPublished),
        Err(error) => return Err(fetch_error(&error)),
    };
    beta_manifest_url(&list, &sources.download_prefix)?.ok_or(UpdateError::NotPublished)
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// The beta manifest of the highest-versioned release in the release list, if any carries one
/// under `download_prefix`.
///
/// # Errors
///
/// `update.fetch_failed` for a list that is no release list.
pub(crate) fn beta_manifest_url(
    list: &[u8],
    download_prefix: &str,
) -> Result<Option<Url>, UpdateError> {
    let releases: Vec<Release> = serde_json::from_slice(list)
        .map_err(|error| UpdateError::Fetch(format!("the release list cannot be read: {error}")))?;
    let newest = releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = parse_version(release.tag_name.trim_start_matches('v'))?;
            let asset = release
                .assets
                .into_iter()
                .find(|asset| asset.name == Channel::Beta.file_name())?;
            let url = Url::parse(&asset.browser_download_url).ok()?;
            (url.scheme() == "https" && url.as_str().starts_with(download_prefix))
                .then_some((version, url))
        })
        .max_by(|(left, _), (right, _)| left.cmp_precedence(right));
    Ok(newest.map(|(_, url)| url))
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
