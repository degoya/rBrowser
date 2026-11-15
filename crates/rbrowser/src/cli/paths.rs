//! The paths on the command line: told apart from URLs, made absolute, and - when rBrowser is
//! started from WSL through interop - a Linux path translated to the Windows side
//! (`/home/u/app` -> `\\wsl.localhost\<distro>\home\u\app`, `/mnt/c/x` -> `C:\x`). The full
//! translation (network mode, other mounts) is `rb-wsl`'s (RB-030-04); this is the simple one
//! the CLI needs.

use std::path::{Component, Path, PathBuf};

/// The share every WSL distribution is reached under from Windows.
const WSL_SHARE: &str = "\\\\wsl.localhost\\";
/// The most characters of a distribution name.
const MAX_DISTRO: usize = 64;

/// Why a path cannot be used.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// A Linux path, but the WSL distribution it belongs to is not known.
    #[error(
        "a Linux path needs its WSL distribution - start rbrowser from that distribution or pass a Windows path"
    )]
    NoDistro,
}

/// Whether a positional argument names a folder rather than a URL: it starts with `.`, `/` or
/// `\`, or with a drive (`C:`, `C:\`, `C:/`).
pub fn looks_like_path(text: &str) -> bool {
    if text.starts_with(['.', '/', '\\']) {
        return true;
    }
    let mut chars = text.chars();
    let drive = chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.next() == Some(':');
    drive && matches!(chars.next(), None | Some('\\' | '/'))
}

/// `text` as the Windows side reads it. With `windows` set, an absolute Linux path is
/// translated: `/mnt/<drive>/...` to the drive, anything else to the WSL share of `distro`.
/// Every other path, and every path on another platform, stays as written.
pub fn translate(text: &str, windows: bool, distro: Option<&str>) -> Result<String, PathError> {
    if !windows || !text.starts_with('/') || text.starts_with("//") {
        return Ok(text.to_owned());
    }
    let parts: Vec<&str> = text.split('/').filter(|part| !part.is_empty()).collect();
    if let [mnt, drive, rest @ ..] = parts.as_slice()
        && *mnt == "mnt"
        && drive.len() == 1
        && drive.chars().all(|c| c.is_ascii_alphabetic())
    {
        return Ok(format!(
            "{}:\\{}",
            drive.to_ascii_uppercase(),
            rest.join("\\")
        ));
    }
    let distro = distro
        .filter(|name| valid_distro(name))
        .ok_or(PathError::NoDistro)?;
    Ok(format!("{WSL_SHARE}{distro}\\{}", parts.join("\\")))
}

/// The WSL distribution of a working folder on the share (`\\wsl.localhost\<distro>\...`,
/// `\\wsl$\<distro>\...`, also in the `\\?\UNC\` form): what Windows sets as the working folder
/// when a Windows program is started from a Linux folder.
pub fn distro_from_cwd(cwd: &str) -> Option<String> {
    let text = cwd.replace('/', "\\");
    let lower = text.to_ascii_lowercase();
    let start = [
        "\\\\?\\unc\\wsl.localhost\\",
        "\\\\?\\unc\\wsl$\\",
        "\\\\wsl.localhost\\",
        "\\\\wsl$\\",
    ]
    .iter()
    .find(|prefix| lower.starts_with(**prefix))
    .map(|prefix| prefix.len())?;
    let name = text.get(start..)?.split('\\').next()?;
    valid_distro(name).then(|| name.to_owned())
}

/// Whether `name` can be a WSL distribution: letters, digits, `. _ -`, not only dots.
pub fn valid_distro(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_DISTRO
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && name.chars().any(|c| c != '.')
}

/// `path` made absolute against `cwd`, with `.` and `..` resolved by the text alone (no link is
/// followed, nothing needs to exist).
pub fn absolute(path: &Path, cwd: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
