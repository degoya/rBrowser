//! The installed Firefox editions (RB-080-32): every `firefox.exe` the registry or the standard
//! folders name, once each, in one fixed search order - the registry's `App Paths\firefox.exe`,
//! the `Mozilla Firefox` folder under Program Files, Program Files (x86) and the local app data
//! (what an empty setting started before RB-080-32, so it still does), the uninstall entries
//! (`InstallLocation`), then the folders of the other three editions - with the edition and the version
//! read from the `application.ini` beside the executable, never by starting Firefox. The
//! enumeration takes its world (environment, file existence, file reading) as parameters, so it
//! is tested on Linux; the registry reads live in [`super::registry`] (Windows only).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file name of the executable on Windows.
pub const EXE_WINDOWS: &str = "firefox.exe";
/// The folder of each edition under Program Files and the local app data, in search order.
pub const FOLDERS: [(&str, Kind); 4] = [
    ("Mozilla Firefox", Kind::Release),
    ("Mozilla Firefox ESR", Kind::Esr),
    ("Firefox Developer Edition", Kind::Developer),
    ("Firefox Nightly", Kind::Nightly),
];
/// The environment variables of the folders the editions install under, in search order.
const FOLDER_VARIABLES: [&str; 3] = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"];
/// How an uninstall entry of each edition is named (the display name starts with it).
const UNINSTALL_PREFIXES: [(&str, Kind); 5] = [
    ("Firefox Developer Edition", Kind::Developer),
    ("Firefox Nightly", Kind::Nightly),
    ("Nightly", Kind::Nightly),
    ("Mozilla Firefox ESR", Kind::Esr),
    ("Mozilla Firefox", Kind::Release),
];
/// Where Firefox installs elsewhere (Linux, macOS).
const ELSEWHERE: [&str; 4] = [
    "/usr/bin/firefox",
    "/usr/local/bin/firefox",
    "/snap/bin/firefox",
    "/Applications/Firefox.app/Contents/MacOS/firefox",
];

/// The edition of an installation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// The release channel.
    Release,
    /// Extended support release.
    Esr,
    /// Developer Edition (the beta channel).
    Developer,
    /// Nightly.
    Nightly,
}

impl Kind {
    /// The id, as the settings list and MCP name it.
    pub const fn id(self) -> &'static str {
        match self {
            Kind::Release => "release",
            Kind::Esr => "esr",
            Kind::Developer => "developer",
            Kind::Nightly => "nightly",
        }
    }

    /// The edition installed in a folder of that name, if it is one of the four.
    fn of_folder(name: &str) -> Option<Kind> {
        FOLDERS
            .iter()
            .find(|(folder, _)| folder.eq_ignore_ascii_case(name))
            .map(|(_, kind)| *kind)
    }

    /// The edition an uninstall entry of that display name belongs to; `None` when it is no
    /// Firefox (the maintenance service, say).
    fn of_uninstall_entry(name: &str) -> Option<Kind> {
        UNINSTALL_PREFIXES
            .iter()
            .find(|(prefix, _)| name.starts_with(prefix))
            .map(|(_, kind)| *kind)
    }
}

/// A place Firefox may be installed, with the edition that place suggests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The executable.
    pub path: PathBuf,
    /// The edition the folder or registry entry names.
    pub kind: Kind,
}

/// One installed Firefox.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    /// The executable.
    pub path: PathBuf,
    /// The edition.
    pub kind: Kind,
    /// `Version=` of its `application.ini`; `None` when the file does not read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// The registry's hits as candidates: the `App Paths` entry (its edition from its folder, a
/// release anywhere else), then the uninstall entries by display name and `InstallLocation`.
pub fn from_registry(
    app_path: Option<PathBuf>,
    uninstall: Vec<(String, PathBuf)>,
) -> (Option<Candidate>, Vec<Candidate>) {
    let app_path = app_path.map(|path| {
        let kind = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| Kind::of_folder(&name.to_string_lossy()))
            .unwrap_or(Kind::Release);
        Candidate { path, kind }
    });
    let uninstall = uninstall
        .into_iter()
        .filter(|(_, location)| !location.as_os_str().is_empty())
        .filter_map(|(name, location)| {
            Kind::of_uninstall_entry(&name).map(|kind| Candidate {
                path: location.join(EXE_WINDOWS),
                kind,
            })
        })
        .collect();
    (app_path, uninstall)
}

/// Every place to look, in order: the registry's candidates, then the four editions' folders
/// under each of [`FOLDER_VARIABLES`] on Windows; the usual paths elsewhere.
pub fn candidates(
    windows: bool,
    app_path: Option<Candidate>,
    uninstall: Vec<Candidate>,
    env: impl Fn(&str) -> Option<OsString>,
) -> Vec<Candidate> {
    let mut found: Vec<Candidate> = app_path.into_iter().collect();
    if !windows {
        found.extend(ELSEWHERE.map(|path| Candidate {
            path: PathBuf::from(path),
            kind: Kind::Release,
        }));
        return found;
    }
    let bases: Vec<PathBuf> = FOLDER_VARIABLES
        .iter()
        .filter_map(|variable| env(variable).filter(|folder| !folder.is_empty()))
        .map(PathBuf::from)
        .collect();
    let in_folders = |kinds: &[(&str, Kind)]| -> Vec<Candidate> {
        bases
            .iter()
            .flat_map(|base| {
                kinds.iter().map(move |(folder, kind)| Candidate {
                    path: base.join(folder).join(EXE_WINDOWS),
                    kind: *kind,
                })
            })
            .collect()
    };
    let (release, others) = FOLDERS.split_at(1);
    found.extend(in_folders(release));
    found.extend(uninstall);
    found.extend(in_folders(others));
    found
}

/// The candidates that exist, each file once (the first spelling stays; `case_insensitive`
/// for Windows paths), in search order, with the version of the `application.ini` beside it.
pub fn installed(
    candidates: Vec<Candidate>,
    case_insensitive: bool,
    exists: impl Fn(&Path) -> bool,
    read: impl Fn(&Path) -> Option<String>,
) -> Vec<Installed> {
    let mut seen: Vec<String> = Vec::new();
    let mut found = Vec::new();
    for candidate in candidates {
        let mut key = candidate.path.to_string_lossy().into_owned();
        if case_insensitive {
            key = key.to_lowercase();
        }
        if seen.contains(&key) || !exists(&candidate.path) {
            continue;
        }
        seen.push(key);
        let version = candidate
            .path
            .parent()
            .and_then(|folder| read(&folder.join("application.ini")))
            .and_then(|ini| version_in(&ini));
        found.push(Installed {
            kind: kind_of(candidate.kind, version.as_deref()),
            path: candidate.path,
            version,
        });
    }
    found
}

/// `Version=` in the `[App]` section of an `application.ini`.
pub fn version_in(ini: &str) -> Option<String> {
    let mut in_app = false;
    for line in ini.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            in_app = section.trim().eq_ignore_ascii_case("App");
            continue;
        }
        if !in_app {
            continue;
        }
        if let Some((name, value)) = line.split_once('=')
            && name.trim() == "Version"
        {
            let value = value.trim();
            return (!value.is_empty()).then(|| value.to_owned());
        }
    }
    None
}

/// The edition `hint` names, refined by what the version says: `...esr` is an ESR, `...a1` a
/// Nightly, wherever the file sits.
pub fn kind_of(hint: Kind, version: Option<&str>) -> Kind {
    match version {
        Some(version) if version.ends_with("esr") => Kind::Esr,
        Some(version) if version.contains('a') => Kind::Nightly,
        _ => hint,
    }
}

#[cfg(test)]
#[path = "installed_tests.rs"]
mod tests;
