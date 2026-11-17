//! rBrowser's self-update, prepared and inactive in phase 1 (RB-090-02; switched on with the move
//! to GitHub, RB-110-05): is there a newer rBrowser, can it be trusted, and how does it replace
//! the running one so that a stop anywhere leaves the old or the new version whole.
//!
//! * [`gate`] - the switch in front of everything: no request, no file while the app's Cargo
//!   feature `self-update` or the setting `updates.enabled` is off; then the check.
//! * [`sign`] - the length-prefixed digest, the signed-document format bound to a domain string,
//!   the replay rules and the compiled-in update root (`rbrowser-update-v1`, a placeholder until
//!   the owner's key exists).
//! * [`manifest`] - the signed update manifest per channel and its verification: size,
//!   signature, schema, channel, freshness, content, in that order.
//! * [`floors`] - the stored replay floor per channel.
//! * [`fetch`] - https at every hop, bounded; [`MemoryFetcher`] for tests.
//! * [`download`] - an artifact streamed to disk and kept only when size and SHA-256 match.
//! * [`install_kind`] - whether this installation updates itself (the portable ZIP only).
//! * [`install`] - the journalled switch next to the app, the updater's run, the roll-back and
//!   the recovery at start; the portable mode's `portable` and `data\` are never moved.
//!
//! No CEF, no config, no plugin host: the updater runs this before the new version is known to
//! start. Adapted from rCommander's `crates/rc-update/` (itself from rDownloader's `rd-update`);
//! every module names its source.

#![warn(unreachable_pub)]

pub mod download;
mod error;
pub mod fetch;
pub mod floors;
pub mod gate;
pub mod install;
pub mod install_kind;
pub mod manifest;
pub mod sign;

/// The time type every check takes, so a caller needs no `chrono` of its own.
pub use chrono::{DateTime, Utc};
pub use download::{Progress, check_space, download_verified};
pub use error::{UPDATE_ERROR_CODES, UpdateError};
pub use fetch::{Fetcher, HttpFetcher, MemoryFetcher};
pub use floors::Floors;
pub use gate::{Gate, check};
pub use install_kind::InstallKind;
pub use manifest::{Artifact, Channel, UpdateManifest};
