//! The networking mode of a distribution: whether a program inside it (Playwright, the CDP
//! tunnel) reaches rBrowser on Windows over `127.0.0.1`.
//!
//! The authority is `wslinfo --networking-mode`, run inside the distribution (WSL 2.0 and
//! later); without it (older WSL, the call failed) the `networkingMode` key of the user's
//! `%USERPROFILE%\.wslconfig` (section `[wsl2]`, the preview releases' `[experimental]`), and
//! without that WSL's default, NAT. The `wsl.networkingModeOverride` setting goes before all of
//! them. The result is kept per distribution ([`ModeCache`]); the mode changes only when WSL
//! restarts.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// WSL's networking modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkingMode {
    /// The default: the distribution sits behind a NAT; Windows' `127.0.0.1` is not its own.
    Nat,
    /// The distribution shares Windows' interfaces; `127.0.0.1` reaches Windows.
    Mirrored,
    /// `virtioproxy`: a proxy in front of a NAT-like network.
    #[serde(rename = "virtioproxy")]
    VirtioProxy,
    /// No network.
    None,
    /// The deprecated bridged mode.
    Bridged,
}

impl NetworkingMode {
    /// Whether `127.0.0.1` inside the distribution reaches a port rBrowser opened on Windows.
    #[must_use]
    pub fn reaches_windows_localhost(self) -> bool {
        self == Self::Mirrored
    }
}

/// Where a [`Detection`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeSource {
    /// The `wsl.networkingModeOverride` setting.
    Override,
    /// `wslinfo --networking-mode` in the distribution.
    Wslinfo,
    /// `%USERPROFILE%\.wslconfig`.
    Wslconfig,
    /// Nothing said otherwise: WSL's default.
    Default,
}

/// A distribution's networking mode and where it was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Detection {
    /// The mode.
    pub mode: NetworkingMode,
    /// Its source.
    pub source: ModeSource,
}

/// The mode a `wslinfo --networking-mode` answer or a `.wslconfig` value names; `None` for
/// anything else (an error message of an older `wslinfo`).
#[must_use]
pub fn parse_mode(text: &str) -> Option<NetworkingMode> {
    let value = text.trim().trim_matches('"').trim();
    [
        ("nat", NetworkingMode::Nat),
        ("mirrored", NetworkingMode::Mirrored),
        ("virtioproxy", NetworkingMode::VirtioProxy),
        ("none", NetworkingMode::None),
        ("bridged", NetworkingMode::Bridged),
    ]
    .into_iter()
    .find(|(name, _)| value.eq_ignore_ascii_case(name))
    .map(|(_, mode)| mode)
}

/// The `networkingMode` of a `.wslconfig` text: the last one of section `[wsl2]`, else the last
/// one of `[experimental]`; keys and sections ignore case, `#` and `;` start comments.
#[must_use]
pub fn from_wslconfig(text: &str) -> Option<NetworkingMode> {
    let mut section = String::new();
    let (mut wsl2, mut experimental) = (None, None);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with(['#', ';']) {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("networkingMode") {
            continue;
        }
        let value = value.split(['#', ';']).next().unwrap_or_default();
        let mode = parse_mode(value);
        match section.as_str() {
            "wsl2" => wsl2 = mode.or(wsl2),
            "experimental" => experimental = mode.or(experimental),
            _ => {}
        }
    }
    wsl2.or(experimental)
}

/// The mode from the sources in their order: the override, `wslinfo`, `.wslconfig`, NAT.
#[must_use]
pub fn resolve(
    override_mode: Option<NetworkingMode>,
    wslinfo: Option<NetworkingMode>,
    wslconfig: Option<NetworkingMode>,
) -> Detection {
    let (mode, source) = override_mode
        .map(|mode| (mode, ModeSource::Override))
        .or_else(|| wslinfo.map(|mode| (mode, ModeSource::Wslinfo)))
        .or_else(|| wslconfig.map(|mode| (mode, ModeSource::Wslconfig)))
        .unwrap_or((NetworkingMode::Nat, ModeSource::Default));
    Detection { mode, source }
}

/// `.wslconfig` in the profile folder `user_profile` (the `USERPROFILE` variable).
#[must_use]
pub fn wslconfig_path(user_profile: Option<&str>) -> Option<PathBuf> {
    user_profile
        .filter(|profile| !profile.is_empty())
        .map(|profile| PathBuf::from(profile).join(".wslconfig"))
}

/// The detected modes, one per distribution (names ignoring case).
#[derive(Debug, Default)]
pub struct ModeCache {
    modes: Mutex<HashMap<String, Detection>>,
}

impl ModeCache {
    /// The mode kept for `distro`.
    #[must_use]
    pub fn get(&self, distro: &str) -> Option<Detection> {
        self.lock().get(&distro.to_ascii_lowercase()).copied()
    }

    /// Keeps `detection` for `distro`.
    pub fn insert(&self, distro: &str, detection: Detection) {
        self.lock().insert(distro.to_ascii_lowercase(), detection);
    }

    /// Forgets `distro`'s mode, so the next lookup detects it again (after a WSL restart).
    pub fn forget(&self, distro: &str) {
        self.lock().remove(&distro.to_ascii_lowercase());
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Detection>> {
        // A panic while holding the lock leaves a map that is still whole.
        self.modes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
#[path = "netmode_tests.rs"]
mod tests;
