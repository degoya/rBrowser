//! What only a Mac has (RB-095-03): Safari, its driver and its version. [`SUPPORTED`] is
//! `false` on Windows and Linux, where the app puts a stand-in engine that says so. The version
//! for Settings > About comes from the app bundle (`defaults read .../Info
//! CFBundleShortVersionString`, [`macos`]) or from a session's `browserVersion`, and is kept
//! ([`cached_version`]), so the About page never waits for a process.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod other;

#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(not(target_os = "macos"))]
use other as imp;

use std::sync::Mutex;

use crate::driver::Edition;

/// Whether this platform has Safari.
pub const SUPPORTED: bool = cfg!(target_os = "macos");

/// The Safari version last found; `None` until one was.
static VERSION: Mutex<Option<String>> = Mutex::new(None);

/// The Safari version last found, without asking anything.
pub fn cached_version() -> Option<String> {
    rb_util::lock(&VERSION).clone()
}

/// Keeps `version` when it is one (`26.1`, `18.6.2`).
pub fn remember_version(version: &str) {
    if let Some(version) = version_of(version) {
        *rb_util::lock(&VERSION) = Some(version);
    }
}

/// Finds the version of `edition`'s bundle and keeps it; `None` outside macOS or when the
/// bundle names none.
pub async fn detect_version(edition: Edition) -> Option<String> {
    let version = imp::bundle_version(edition).await?;
    remember_version(&version);
    version_of(&version)
}

/// The version in the output of `defaults read`: its first line when it looks like one.
pub fn version_of(output: &str) -> Option<String> {
    let line = output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let plain = line.len() <= 32
        && line.chars().next().is_some_and(|c| c.is_ascii_digit())
        && line.chars().all(|c| c.is_ascii_digit() || c == '.');
    plain.then(|| line.to_owned())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
