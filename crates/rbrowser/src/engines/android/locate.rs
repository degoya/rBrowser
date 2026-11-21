//! Which `adb` runs (RB-095-02, the owner's decision of 2026-11-21): `engines.android.adbPath`
//! when set, else the first `adb` in an absolute folder of `PATH`, in `ANDROID_HOME` and `ANDROID_SDK_ROOT`
//! (`platform-tools/`), in the usual SDK folders (`%LOCALAPPDATA%\Android\Sdk`,
//! `~/Library/Android/sdk`, `~/Android/Sdk`), and last the Android Platform Tools rBrowser
//! downloaded after the person agreed (`super::adb_tool`). The search is pure over [`Env`] and a
//! file check, so the tests need no file system. A path the person set that names no file is
//! refused ([`Missing::SettingNotAFile`]), never replaced by another adb behind their back.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// The SDK's folder of `adb`.
const PLATFORM_TOOLS: &str = "platform-tools";

/// What the search reads from the process.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Env {
    /// `PATH`.
    pub path: Option<OsString>,
    /// `ANDROID_HOME`.
    pub android_home: Option<PathBuf>,
    /// `ANDROID_SDK_ROOT`.
    pub sdk_root: Option<PathBuf>,
    /// `LOCALAPPDATA` (Windows).
    pub local_app_data: Option<PathBuf>,
    /// The home folder (`USERPROFILE` on Windows, `HOME` elsewhere).
    pub home: Option<PathBuf>,
}

impl Env {
    /// The variables of this process.
    pub fn from_process() -> Self {
        let dir = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Self {
            path: std::env::var_os("PATH"),
            android_home: dir("ANDROID_HOME"),
            sdk_root: dir("ANDROID_SDK_ROOT"),
            local_app_data: dir("LOCALAPPDATA"),
            home: dir("USERPROFILE").or_else(|| dir("HOME")),
        }
    }
}

/// Where an `adb` was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// `engines.android.adbPath`.
    Setting,
    /// A folder of `PATH`.
    Path,
    /// `ANDROID_HOME`.
    AndroidHome,
    /// `ANDROID_SDK_ROOT`.
    SdkRoot,
    /// A usual SDK folder.
    UsualSdk,
    /// The Platform Tools rBrowser downloaded.
    Managed,
}

/// The `adb` found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    /// The program.
    pub path: PathBuf,
    /// Where it was found.
    pub source: Source,
}

/// Why no `adb` runs.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Missing {
    /// `engines.android.adbPath` names no file.
    #[error("engines.android.adbPath names no file: {}", .0.display())]
    SettingNotAFile(PathBuf),
    /// No adb anywhere.
    #[error("no adb was found (PATH, ANDROID_HOME, ANDROID_SDK_ROOT, the usual SDK folders)")]
    NotFound,
}

/// `adb.exe` on Windows, `adb` elsewhere.
pub const fn program_name(windows: bool) -> &'static str {
    if windows { "adb.exe" } else { "adb" }
}

/// Every place asked, in order, with its source; `program` is [`program_name`].
pub fn candidates(
    setting: Option<&str>,
    env: &Env,
    managed: Option<&Path>,
    program: &str,
) -> Vec<(PathBuf, Source)> {
    let mut list = Vec::new();
    if let Some(path) = setting {
        list.push((PathBuf::from(path), Source::Setting));
    }
    if let Some(path) = &env.path {
        // A relative or empty entry is a folder of the current directory: never asked.
        list.extend(
            std::env::split_paths(path)
                .filter(|dir| dir.is_absolute())
                .map(|dir| (dir.join(program), Source::Path)),
        );
    }
    let tools = |sdk: &Path| sdk.join(PLATFORM_TOOLS).join(program);
    if let Some(home) = &env.android_home {
        list.push((tools(home), Source::AndroidHome));
    }
    if let Some(root) = &env.sdk_root {
        list.push((tools(root), Source::SdkRoot));
    }
    if let Some(local) = &env.local_app_data {
        list.push((tools(&local.join("Android").join("Sdk")), Source::UsualSdk));
    }
    if let Some(home) = &env.home {
        let mac = home.join("Library").join("Android").join("sdk");
        list.push((tools(&mac), Source::UsualSdk));
        list.push((tools(&home.join("Android").join("Sdk")), Source::UsualSdk));
    }
    if let Some(managed) = managed {
        list.push((managed.to_path_buf(), Source::Managed));
    }
    list
}

/// The first candidate that is a file.
///
/// # Errors
///
/// The setting names no file, or there is none.
pub fn locate(
    setting: Option<&str>,
    env: &Env,
    managed: Option<&Path>,
    program: &str,
    is_file: impl Fn(&Path) -> bool,
) -> Result<Found, Missing> {
    if let Some(path) = setting
        && !is_file(Path::new(path))
    {
        return Err(Missing::SettingNotAFile(PathBuf::from(path)));
    }
    let found = candidates(setting, env, managed, program)
        .into_iter()
        .find(|(path, _)| is_file(path))
        .map(|(path, source)| Found { path, source });
    // RB-096-10: which adb runs, for a device that does not show.
    match &found {
        Some(found) => {
            tracing::debug!(path = %found.path.display(), source = ?found.source, "adb in use");
        }
        None => tracing::debug!("no adb was found"),
    }
    found.ok_or(Missing::NotFound)
}

#[cfg(test)]
#[path = "locate_tests.rs"]
mod tests;
