//! The `capture` section of the settings (Settings > Capture > Screenshots and Cookie banners &
//! animations, `schema/parts/capture.json`): read from the project config (with `config.local.json`) and
//! the user settings, the project winning per key as everywhere (`docs/settings.md`). A value
//! that does not fit falls back to its default and is named, so one bad key never loses the
//! others.
//!
//! The folder: a project may only name a real folder inside the project (a checked-in config
//! must not write elsewhere): no `..`, no folder link on the way (a link can be checked in), not
//! inside `.git/` or `.rbrowser/` but for `.rbrowser/out/`. The user settings may also name an
//! absolute folder of their own, but no network share or device path.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::naming::{DEFAULT_PATTERN, validate_pattern};
use crate::prepare::{CleanSettings, MAX_SETTLE_MS, parse_selectors};
use crate::{Delay, VisualError};

/// The section's name in both files.
pub const SECTION: &str = "capture";
/// The folder below the project root when `capture.dir` is not set.
pub const DEFAULT_DIR: [&str; 3] = [".rbrowser", "out", "screenshots"];
/// `capture.elementPadding` when not set, in CSS px.
pub const DEFAULT_PADDING: u32 = 16;
/// The largest `capture.elementPadding`.
pub const MAX_PADDING: u32 = 512;

/// Which file a value came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// `.rbrowser/config.json` or `config.local.json`.
    Project,
    /// The user settings.
    User,
}

/// The screenshot settings in effect.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureSettings {
    /// `capture.dir` and where it was set; the project's `.rbrowser/out/screenshots` when
    /// `None`.
    pub dir: Option<(Origin, String)>,
    /// `capture.pathPattern`.
    pub path_pattern: String,
    /// `capture.elementPadding`, CSS px around an element.
    pub element_padding: u32,
    /// `capture.defaultDelay`.
    pub default_delay: Delay,
    /// `capture.overviewLabels`: the overview shows label, size and engine under each view.
    pub overview_labels: bool,
    /// Capture > Cookie banners & animations (RB-080-12).
    pub clean: CleanSettings,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            dir: None,
            path_pattern: DEFAULT_PATTERN.to_owned(),
            element_padding: DEFAULT_PADDING,
            default_delay: Delay::None,
            overview_labels: true,
            clean: CleanSettings::default(),
        }
    }
}

impl CaptureSettings {
    /// The settings from the `capture` sections of the project config and the user settings,
    /// and a line per value that was refused (for the log).
    pub fn resolve(project: Option<&Value>, user: Option<&Value>) -> (Self, Vec<String>) {
        let mut settings = Self::default();
        let mut problems = Vec::new();
        // The user first, then the project over it.
        for (origin, section) in [(Origin::User, user), (Origin::Project, project)] {
            let Some(section) = section.and_then(Value::as_object) else {
                continue;
            };
            for (key, value) in section {
                if let Err(reason) = settings.set(origin, key, value) {
                    problems.push(format!("{SECTION}.{key}: {reason}"));
                }
            }
        }
        (settings, problems)
    }

    fn set(&mut self, origin: Origin, key: &str, value: &Value) -> Result<(), String> {
        if value.is_null() {
            return Ok(());
        }
        match key {
            "dir" => {
                let text = value.as_str().ok_or("not a string")?.trim();
                self.dir = (!text.is_empty()).then(|| (origin, text.to_owned()));
            }
            "pathPattern" => {
                let text = value.as_str().ok_or("not a string")?;
                validate_pattern(text).map_err(|error| error.to_string())?;
                text.clone_into(&mut self.path_pattern);
            }
            "elementPadding" => {
                let padding = value
                    .as_u64()
                    .and_then(|padding| u32::try_from(padding).ok())
                    .filter(|padding| *padding <= MAX_PADDING)
                    .ok_or_else(|| format!("not a number from 0 to {MAX_PADDING}"))?;
                self.element_padding = padding;
            }
            "defaultDelay" => {
                self.default_delay = value
                    .as_u64()
                    .and_then(Delay::from_secs)
                    .ok_or("not 0, 5 or 10")?;
            }
            "overviewLabels" => self.overview_labels = value.as_bool().ok_or("not a boolean")?,
            "hideBanners" => self.clean.hide_banners = value.as_bool().ok_or("not a boolean")?,
            "hideSelectors" => self.clean.hide_selectors = parse_selectors(value)?,
            "useBuiltinList" => {
                self.clean.use_builtin_list = value.as_bool().ok_or("not a boolean")?;
            }
            "freezeAnimations" => {
                self.clean.freeze_animations = value.as_bool().ok_or("not a boolean")?;
            }
            "settleMs" => {
                self.clean.settle_ms = value
                    .as_u64()
                    .and_then(|ms| u32::try_from(ms).ok())
                    .filter(|ms| *ms <= MAX_SETTLE_MS)
                    .ok_or_else(|| format!("not a number from 0 to {MAX_SETTLE_MS}"))?;
            }
            // A key of a later job (RB-050-04 and on) is not this one's to judge.
            _ => {}
        }
        Ok(())
    }

    /// The folder the screenshots of `project_root` go to.
    pub fn output_dir(&self, project_root: &Path) -> Result<PathBuf, VisualError> {
        let Some((origin, dir)) = &self.dir else {
            return real_folder(project_root, &DEFAULT_DIR, &DEFAULT_DIR.join("/"));
        };
        if *origin == Origin::User {
            if is_share_or_device(dir) {
                return Err(VisualError::Folder(format!(
                    "{dir} is a network share or a device, not a folder"
                )));
            }
            if Path::new(dir).is_absolute() {
                return Ok(PathBuf::from(dir));
            }
        }
        inside(project_root, dir)
    }
}

/// A UNC path (`\\host\share`, `//host/share`, `\\?\UNC\...`) or a device path (`\\.\...`,
/// `\\?\...`): two leading separators of either kind.
fn is_share_or_device(dir: &str) -> bool {
    let mut chars = dir.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some('/' | '\\'), Some('/' | '\\'))
    )
}

/// `dir` below `root`: relative, without `..`, a drive or a stream name, not in `.git/` or in
/// `.rbrowser/` (but for `.rbrowser/out/`), and a real folder on the way (see [`real_folder`]).
fn inside(root: &Path, dir: &str) -> Result<PathBuf, VisualError> {
    let refused = || VisualError::Folder(format!("{dir} is not a folder inside the project"));
    if dir.starts_with(['/', '\\']) || dir.contains(':') {
        return Err(refused());
    }
    let mut parts = Vec::new();
    for part in dir.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return Err(refused()),
            part => parts.push(part),
        }
    }
    // Windows ignores case and trailing dots and spaces: `.GIT.` is `.git`.
    let name = |index: usize| {
        parts
            .get(index)
            .map(|part| part.trim_end_matches(['.', ' ']).to_ascii_lowercase())
    };
    let protected = match name(0).as_deref() {
        Some(".git") => true,
        Some(".rbrowser") => name(1).as_deref() != Some("out"),
        _ => false,
    };
    if protected {
        return Err(VisualError::Folder(format!(
            "{dir} is inside .git or .rbrowser, which hold the project's own files"
        )));
    }
    real_folder(root, &parts, dir)
}

/// `root` joined with `parts`, each one that exists a real folder - no link (which may lead out
/// of the project), no file. Missing parts are created when a screenshot is written.
fn real_folder(root: &Path, parts: &[&str], shown: &str) -> Result<PathBuf, VisualError> {
    let mut path = root.to_path_buf();
    for (index, part) in parts.iter().enumerate() {
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(VisualError::Folder(format!(
                    "{shown} leads through a link, which may leave the project"
                )));
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(VisualError::Folder(format!("{shown} is not a folder"))),
            // Neither it nor the rest exists yet.
            Err(_) => {
                path.extend(&parts[index + 1..]);
                break;
            }
        }
    }
    Ok(path)
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
