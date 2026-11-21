//! rBrowser's self-update (RB-090-02, switched on with RB-090-14): is there a newer rBrowser, can
//! it be trusted, and how does it replace the running one so that a stop anywhere leaves the old
//! or the new version whole.
//!
//! * [`gate`] - the switch in front of everything: no request, no file while the app's Cargo
//!   feature `self-update` or the setting `updates.enabled` is off; then the check.
//! * [`sources`] - where the manifests are published (the latest release, the release list for
//!   beta) and one check of every manifest a channel reads.
//! * [`offer`] - which verified release this installation is offered, and the channel a build
//!   reads.
//! * [`auto_install`] - whether quitting installs a ready download (`updates.autoInstall`).
//! * [`sign`] - the length-prefixed digest, the signed-document format bound to a domain string,
//!   the replay rules and the compiled-in update root (`rbrowser-update-v1`, the owner's key).
//! * [`manifest`] - the signed update manifest per channel and its verification: size,
//!   signature, schema, channel, freshness, content, in that order.
//! * [`floors`] - the stored replay floor per channel.
//! * [`fetch`] - https at every hop, bounded; [`MemoryFetcher`] for tests.
//! * [`download`] - an artifact streamed to disk and kept only when size and SHA-256 match.
//! * [`install_kind`] - whether this installation updates itself (the portable ZIP only), shows a
//!   package manager's command, or offers the download.
//! * [`install`] - the app's hand-over, the journalled switch next to the app, the updater's run
//!   (`rbrowser-updater`), the roll-back and the recovery at start; the portable mode's
//!   `portable` and `data\` are never moved.
//!
//! No CEF, no config, no plugin host: the updater runs this before the new version is known to
//! start. Adapted from rCommander's `crates/rc-update/` (itself from rDownloader's `rd-update`);
//! every module names its source.

#![warn(unreachable_pub)]

pub mod auto_install;
pub mod download;
mod error;
pub mod fetch;
pub mod floors;
pub mod gate;
pub mod install;
pub mod install_kind;
pub mod manifest;
pub mod offer;
pub mod sign;
pub mod sources;
pub mod utc;

pub use download::{Progress, already_downloaded, check_space, download_verified};
pub use error::{UPDATE_ERROR_CODES, UpdateError};
pub use fetch::{Fetcher, HttpFetcher, MemoryFetcher};
pub use floors::Floors;
pub use gate::{Gate, check};
pub use install_kind::{InstallKind, UpdateWay};
pub use manifest::{Artifact, Channel, UpdateManifest};
pub use offer::{Offer, Target, effective_channel, newest_offer};
pub use sources::{CheckReport, Sources, check_channel};
/// The time type every check takes, always in UTC, so a caller needs no `time` of its own.
pub use time::OffsetDateTime;
