//! Which packages a start loads (RB-091-04, after rDownloader's RD-160-05 and RD-1260-02): the
//! bundle beside the executable is where first-party plugins come from, not what every start
//! installs. A bundled package loads only when the person chose its plugin - the first-run
//! wizard, the catalogue, the recommended set of a fresh installation - and the others are
//! *available*. A copy in the person's plugin folder that the repository delivered stands in for
//! the bundled package when it is newer and declares the same key - the loader then verifies the
//! copy and loads the bundled package instead when the copy is refused, and counts the copy as
//! first-party only when it is signed (review 2); a bundle as new or newer than the copy wins (an application update). A copy
//! signed by another key never stands in: the bundled package loads first and the copy is refused
//! as a duplicate.
//!
//! Decided from what each manifest says of itself, read without verifying it; every package that
//! loads is verified afterwards as before, so a claim that does not hold is refused there.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::loader::PluginSource;
use crate::manifest::PluginManifest;

/// What a package's manifest says of itself, unverified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Declared {
    pub(crate) id: String,
    pub(crate) version: semver::Version,
    pub(crate) public_key: String,
}

impl Declared {
    pub(crate) fn of(manifest: &PluginManifest) -> Self {
        Self {
            id: manifest.id.clone(),
            version: manifest.version.clone(),
            public_key: manifest.public_key.clone(),
        }
    }
}

/// One package found, in loading order.
#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    pub(crate) source: PluginSource,
    pub(crate) path: PathBuf,
    /// `None` for a manifest that does not read, and for a project package (never read here).
    pub(crate) declared: Option<Declared>,
}

/// What a start loads.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Choice {
    /// In loading order; where the person's copy stands in for a bundled package, that package's
    /// path, loaded instead when the copy is refused.
    pub(crate) load: Vec<(PluginSource, PathBuf, Option<PathBuf>)>,
    /// Bundled packages whose plugin the person did not choose.
    pub(crate) available: Vec<PathBuf>,
    /// Packages left out for another copy of the same plugin.
    pub(crate) superseded: Vec<PathBuf>,
}

/// The packages of `candidates` a start loads; `chosen` names the bundled plugins the person
/// chose, `None` loads every bundled package.
pub(crate) fn choose(candidates: Vec<Candidate>, chosen: Option<&BTreeSet<String>>) -> Choice {
    let wanted = |id: &str| chosen.is_none_or(|chosen| chosen.contains(id));
    // The chosen bundled package of each plugin, by id: the first one found.
    let mut bundled: BTreeMap<&str, (usize, &Declared)> = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.source != PluginSource::Bundled {
            continue;
        }
        if let Some(declared) = &candidate.declared
            && wanted(&declared.id)
        {
            bundled
                .entry(declared.id.as_str())
                .or_insert((index, declared));
        }
    }
    let mut superseded: BTreeSet<usize> = BTreeSet::new();
    // The bundled packages held back as the fallback of a copy, and each copy's bundled package.
    let mut held: BTreeSet<usize> = BTreeSet::new();
    let mut stand_ins: BTreeMap<usize, usize> = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.source != PluginSource::User {
            continue;
        }
        let Some(declared) = &candidate.declared else {
            continue;
        };
        let Some((shipped_index, shipped)) = bundled.get(declared.id.as_str()) else {
            continue;
        };
        if declared.public_key != shipped.public_key {
            continue;
        }
        if declared.version > shipped.version {
            held.insert(*shipped_index);
            stand_ins.insert(index, *shipped_index);
        } else {
            superseded.insert(index);
        }
    }
    let paths: Vec<PathBuf> = candidates
        .iter()
        .map(|candidate| candidate.path.clone())
        .collect();
    let mut choice = Choice::default();
    for (index, candidate) in candidates.into_iter().enumerate() {
        if superseded.contains(&index) {
            choice.superseded.push(candidate.path);
            continue;
        }
        if held.contains(&index) {
            continue;
        }
        let unchosen = candidate.source == PluginSource::Bundled
            && candidate
                .declared
                .as_ref()
                .is_some_and(|declared| !wanted(&declared.id));
        if unchosen {
            choice.available.push(candidate.path);
            continue;
        }
        let fallback = stand_ins
            .get(&index)
            .and_then(|shipped| paths.get(*shipped).cloned());
        choice
            .load
            .push((candidate.source, candidate.path, fallback));
    }
    choice
}

#[cfg(test)]
#[path = "loader_choice_tests.rs"]
mod tests;
