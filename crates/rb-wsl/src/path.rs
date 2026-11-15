//! Paths between Windows and a WSL distribution, in both directions.
//!
//! A folder inside a distribution is a UNC share on Windows: `/home/u/app` in `Ubuntu` is
//! `\\wsl.localhost\Ubuntu\home\u\app` (`\\wsl$\...` on older Windows 10 builds, still accepted
//! everywhere). A Windows drive is mounted inside the distribution: `C:\x` is `/mnt/c/x`, under
//! the automount root of the distribution's `wsl.conf` (`/mnt` unless it says otherwise).
//!
//! Pure string work, so every rule is tested on every platform. Names pass through as they are
//! (spaces, non-ASCII letters, `#`); `.` names are dropped, and a `..` name is refused in both
//! directions, so a translated path never leaves the folder it names. Adapted from rCommander's
//! `rc-vfs::parse_wsl_unc` and `rc-term::wsl_path`.

/// The UNC host Windows 11 and current Windows 10 serve the distributions under.
pub const WSL_HOST: &str = "wsl.localhost";
/// The older host name, still accepted by every Windows version.
pub const WSL_LEGACY_HOST: &str = "wsl$";
/// WSL's default automount root: drive `C:` is `/mnt/c`.
pub const DEFAULT_MOUNT_ROOT: &str = "/mnt";

/// The host a translated UNC path is written with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UncHost {
    /// `\\wsl.localhost\...`.
    #[default]
    Localhost,
    /// `\\wsl$\...`, for Windows builds before `wsl.localhost` existed.
    Legacy,
}

impl UncHost {
    /// The host name as written in the path.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Localhost => WSL_HOST,
            Self::Legacy => WSL_LEGACY_HOST,
        }
    }
}

/// Why a path has no counterpart on the other side.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// Not a `\\wsl.localhost\...` or `\\wsl$\...` path.
    #[error("not a path inside a WSL distribution")]
    NotWsl,
    /// Not a name WSL serves a share under.
    #[error("not a WSL distribution name: {0:?}")]
    InvalidDistro(String),
    /// A relative path, a drive-relative one (`C:x`) or an empty one.
    #[error("the path is not absolute")]
    NotAbsolute,
    /// A `..` name, refused so a path cannot climb out of what it names.
    #[error("a path with `..` is not accepted")]
    ParentComponent,
    /// A Linux name holding a backslash, which Windows reads as a separator.
    #[error("a Linux name holding a backslash has no Windows path")]
    Backslash,
    /// A network share, which WSL does not mount.
    #[error("a network share has no path inside WSL")]
    NetworkShare,
    /// The path is in another distribution's share.
    #[error("the path belongs to the WSL distribution {found:?}, not {expected:?}")]
    OtherDistro {
        /// The distribution translated for.
        expected: String,
        /// The distribution the path names.
        found: String,
    },
}

impl PathError {
    /// The i18n key of the message a user reads.
    #[must_use]
    pub fn i18n_key(&self) -> &'static str {
        match self {
            Self::NotWsl => "wsl.error.path.notWsl",
            Self::InvalidDistro(_) => "wsl.error.path.invalidDistro",
            Self::NotAbsolute => "wsl.error.path.notAbsolute",
            Self::ParentComponent => "wsl.error.path.parent",
            Self::Backslash => "wsl.error.path.backslash",
            Self::NetworkShare => "wsl.error.path.networkShare",
            Self::OtherDistro { .. } => "wsl.error.path.otherDistro",
        }
    }
}

/// A path inside a distribution, split out of its UNC form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WslPath {
    /// The distribution as the share spells it.
    pub distro: String,
    /// The absolute Linux path, `/` separated, without a trailing `/` (except the root).
    pub linux_path: String,
}

/// Whether `name` can be a distribution's share name.
#[must_use]
pub fn valid_distro(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.chars().any(|c| {
            c.is_control() || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
}

/// Whether `text` is a path in a distribution's share (any host spelling, `/` or `\`).
#[must_use]
pub fn is_wsl_unc(text: &str) -> bool {
    split_unc(&text.replace('/', "\\")).is_some_and(|(host, _)| wsl_host(host))
}

/// Splits `\\wsl.localhost\<distro>\...`, `\\wsl$\<distro>\...`, their `\\?\UNC\` forms and the
/// same with `/` into distribution and Linux path.
///
/// # Errors
///
/// [`PathError::NotWsl`] for any other path, [`PathError::InvalidDistro`] for a share name WSL
/// cannot have, [`PathError::ParentComponent`] for a `..` name.
pub fn parse_unc(text: &str) -> Result<WslPath, PathError> {
    let text = text.replace('/', "\\");
    let Some((host, rest)) = split_unc(&text) else {
        return Err(PathError::NotWsl);
    };
    if !wsl_host(host) {
        return Err(PathError::NotWsl);
    }
    let (distro, inner) = rest.split_once('\\').unwrap_or((rest, ""));
    if !valid_distro(distro) {
        return Err(PathError::InvalidDistro(distro.to_owned()));
    }
    Ok(WslPath {
        distro: distro.to_owned(),
        linux_path: format!("/{}", names(inner, '\\')?.join("/")),
    })
}

/// The UNC path of `linux_path` in `distro`: `("Ubuntu", "/home/u")` is
/// `\\wsl.localhost\Ubuntu\home\u`.
///
/// # Errors
///
/// [`PathError::InvalidDistro`], [`PathError::NotAbsolute`], [`PathError::Backslash`] or
/// [`PathError::ParentComponent`].
pub fn unc_path(distro: &str, linux_path: &str, host: UncHost) -> Result<String, PathError> {
    if !valid_distro(distro) {
        return Err(PathError::InvalidDistro(distro.to_owned()));
    }
    let names = linux_names(linux_path)?;
    Ok(format!(
        r"\\{}\{distro}\{}",
        host.as_str(),
        names.join("\\")
    ))
}

/// The absolute Linux path `linux` in its plain form: `/` separated, no empty or `.` names, no
/// trailing `/` (except the root).
///
/// # Errors
///
/// [`PathError::NotAbsolute`], [`PathError::Backslash`] or [`PathError::ParentComponent`].
pub fn normalize_linux(linux: &str) -> Result<String, PathError> {
    Ok(format!("/{}", linux_names(linux)?.join("/")))
}

/// Translates between Windows and Linux paths for one distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translator {
    distro: String,
    mount_root: Vec<String>,
    host: UncHost,
}

impl Translator {
    /// A translator for `distro` with the default mount root and `wsl.localhost`.
    ///
    /// # Errors
    ///
    /// [`PathError::InvalidDistro`] when `distro` cannot be a share name.
    pub fn new(distro: &str) -> Result<Self, PathError> {
        if !valid_distro(distro) {
            return Err(PathError::InvalidDistro(distro.to_owned()));
        }
        Ok(Self {
            distro: distro.to_owned(),
            mount_root: root_names(DEFAULT_MOUNT_ROOT),
            host: UncHost::Localhost,
        })
    }

    /// The same with the automount root of the distribution's `wsl.conf` (`[automount] root`).
    #[must_use]
    pub fn with_mount_root(mut self, root: &str) -> Self {
        self.mount_root = root_names(root);
        self
    }

    /// The same, writing UNC paths with `host`.
    #[must_use]
    pub fn with_host(mut self, host: UncHost) -> Self {
        self.host = host;
        self
    }

    /// The distribution translated for.
    #[must_use]
    pub fn distro(&self) -> &str {
        &self.distro
    }

    /// The Linux path of the Windows path `windows`: a path in this distribution's share is the
    /// path behind it (the name matched ignoring case, as Windows does), a drive path is its
    /// mount.
    ///
    /// # Errors
    ///
    /// [`PathError::OtherDistro`], [`PathError::NetworkShare`], [`PathError::NotAbsolute`],
    /// [`PathError::ParentComponent`] or [`PathError::InvalidDistro`].
    pub fn to_linux(&self, windows: &str) -> Result<String, PathError> {
        let text = windows.replace('/', "\\");
        if let Some((host, _)) = split_unc(&text) {
            if !wsl_host(host) {
                return Err(PathError::NetworkShare);
            }
            let inside = parse_unc(&text)?;
            if !inside.distro.eq_ignore_ascii_case(&self.distro) {
                return Err(PathError::OtherDistro {
                    expected: self.distro.clone(),
                    found: inside.distro,
                });
            }
            return Ok(inside.linux_path);
        }
        let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
        if text.starts_with(r"\\") {
            return Err(PathError::NetworkShare);
        }
        let (letter, rest) = split_drive(text).ok_or(PathError::NotAbsolute)?;
        if !rest.is_empty() && !rest.starts_with('\\') {
            return Err(PathError::NotAbsolute);
        }
        let mut parts = self.mount_root.clone();
        parts.push(letter.to_ascii_lowercase().to_string());
        parts.extend(names(rest, '\\')?.into_iter().map(str::to_owned));
        Ok(format!("/{}", parts.join("/")))
    }

    /// The Windows path of the Linux path `linux`: a path under a drive mount is that drive's
    /// path, every other path is in this distribution's share. Only a lowercase single letter
    /// below the mount root is a drive (`/mnt/c`), as WSL mounts them.
    ///
    /// # Errors
    ///
    /// [`PathError::NotAbsolute`], [`PathError::Backslash`] or [`PathError::ParentComponent`].
    pub fn to_windows(&self, linux: &str) -> Result<String, PathError> {
        let names = linux_names(linux)?;
        let root = self.mount_root.len();
        let under_root = names.len() > root
            && names
                .iter()
                .zip(&self.mount_root)
                .all(|(name, root_name)| *name == root_name.as_str());
        if under_root
            && let [drive, rest @ ..] = &names[root..]
            && let [letter] = drive.as_bytes()
            && letter.is_ascii_lowercase()
        {
            return Ok(format!(
                r"{}:\{}",
                char::from(*letter).to_ascii_uppercase(),
                rest.join("\\")
            ));
        }
        unc_path(&self.distro, linux, self.host)
    }
}

/// `(host, rest)` of a `\\host\rest` or `\\?\UNC\host\rest` path; `None` for anything else,
/// including a `\\?\C:\` path.
fn split_unc(text: &str) -> Option<(&str, &str)> {
    let rest = match text.strip_prefix(r"\\?\") {
        Some(verbatim) => {
            let unc = verbatim.get(..4)?;
            if !unc.eq_ignore_ascii_case(r"UNC\") {
                return None;
            }
            &verbatim[4..]
        }
        None => text.strip_prefix(r"\\")?,
    };
    Some(rest.split_once('\\').unwrap_or((rest, "")))
}

fn wsl_host(host: &str) -> bool {
    host.eq_ignore_ascii_case(WSL_HOST) || host.eq_ignore_ascii_case(WSL_LEGACY_HOST)
}

/// The drive letter and what follows `X:`.
fn split_drive(text: &str) -> Option<(char, &str)> {
    let mut chars = text.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    (chars.next() == Some(':')).then_some((letter, chars.as_str()))
}

/// The names of `text` split at `separator`: empty and `.` names dropped, `..` refused.
fn names(text: &str, separator: char) -> Result<Vec<&str>, PathError> {
    let mut out = Vec::new();
    for name in text.split(separator) {
        match name {
            "" | "." => {}
            ".." => return Err(PathError::ParentComponent),
            name => out.push(name),
        }
    }
    Ok(out)
}

/// The names of the absolute Linux path `linux`.
fn linux_names(linux: &str) -> Result<Vec<&str>, PathError> {
    if !linux.starts_with('/') {
        return Err(PathError::NotAbsolute);
    }
    if linux.contains('\\') {
        return Err(PathError::Backslash);
    }
    names(linux, '/')
}

/// The names of an automount root; a root that is not a plain absolute path counts as `/mnt`.
fn root_names(root: &str) -> Vec<String> {
    match linux_names(root) {
        Ok(names) => names.into_iter().map(str::to_owned).collect(),
        Err(_) => vec![DEFAULT_MOUNT_ROOT.trim_start_matches('/').to_owned()],
    }
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
