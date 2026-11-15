//! The replay floors: the highest manifest sequence this installation accepted per channel,
//! stored in `<data>/update/floors.json` before anything acts on a check. A manifest below it is
//! a replay of an older one (`manifest::verify`); without the file a first contact accepts any
//! sequence the manifest's expiry still allows.
//!
//! Adapted from rCommander's `crates/rc-update/src/check.rs` (`Floors`) and `state.rs` (RB-090-02):
//! a file of its own instead of a database setting.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::install::{durable, update_dir};
use crate::manifest::Channel;

/// Inside `<data>/update`.
pub const FLOORS_FILE: &str = "floors.json";

/// The highest sequence accepted per channel.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Floors {
    #[serde(default)]
    pub stable: Option<u64>,
    #[serde(default)]
    pub beta: Option<u64>,
}

impl Floors {
    /// `<data>/update/floors.json`.
    #[must_use]
    pub fn path(data: &Path) -> PathBuf {
        update_dir(data).join(FLOORS_FILE)
    }

    /// The stored floors; none stored, or unreadable, is a first contact (a warning, never a
    /// failure: the manifests' expiry bounds a replay then).
    #[must_use]
    pub fn load(data: &Path) -> Self {
        let path = Self::path(data);
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                tracing::warn!(%error, "the update floors cannot be read; they start afresh");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Writes the floors atomically.
    ///
    /// # Errors
    ///
    /// When the file cannot be written; a check must not act on a manifest whose floor is not
    /// stored.
    pub fn store(&self, data: &Path) -> Result<()> {
        let path = Self::path(data);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        durable::write_atomically(&path, &serde_json::to_vec_pretty(self)?)
    }

    /// `channel`'s floor.
    #[must_use]
    pub fn get(&self, channel: Channel) -> Option<u64> {
        match channel {
            Channel::Stable => self.stable,
            Channel::Beta => self.beta,
        }
    }

    /// Raises `channel`'s floor to `sequence`; never lowers it.
    pub fn raise(&mut self, channel: Channel, sequence: u64) {
        let floor = match channel {
            Channel::Stable => &mut self.stable,
            Channel::Beta => &mut self.beta,
        };
        *floor = Some(floor.map_or(sequence, |known| known.max(sequence)));
    }
}

#[cfg(test)]
#[path = "floors_tests.rs"]
mod tests;
