//! The one place designs come from: `.rbrowser/designs/` of the open project.
//!
//! A design is named relative to the config folder (`designs/mobile.png`, the form of the setting
//! `image-overlay.designs`; `.rbrowser/designs/mobile.png` is taken too). Anything else - a path
//! outside `designs/`, `..` or `.`, an absolute path or a drive, a backslash, a hidden folder, a
//! Windows short name, a character outside a plain file name, another type than PNG, JPEG or
//! WebP - is refused here, before the host is asked; the host's own path rule
//! (`project.read`: no link followed, never `config.local.json`) stands behind it.
//! `.rbrowser/designs/` is no folder workspace trust protects (`rb_config::trust::PROTECTED_DIRS`):
//! designs are images, never code.

use crate::image::Kind;

/// The folder below `.rbrowser/` designs live in.
pub const DESIGNS_DIR: &str = "designs";
/// The same folder, relative to the project (`project.list`).
pub const DESIGNS_PROJECT_DIR: &str = ".rbrowser/designs";
/// The longest design path, in characters.
pub const MAX_PATH_CHARS: usize = 255;

/// Why a design path is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathRefusal {
    /// Not below `.rbrowser/designs/`, or not a plain relative path.
    Outside,
    /// Not a PNG, JPEG or WebP file name.
    BadType,
}

/// A checked design path: `designs/<name>` with plain segments and an image extension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignPath {
    path: String,
    kind: Kind,
}

/// Whether `segment` is a plain file or folder name: letters, digits, space and `._-@+()`, not
/// hidden, no `.`/`..`, no Windows short name (`~` and a digit).
fn plain(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.starts_with('.')
        && !segment.contains('~')
        && segment.chars().all(|c| {
            c.is_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-' | '@' | '+' | '(' | ')')
        })
}

impl DesignPath {
    /// Checks `text`, a path relative to `.rbrowser/` or to the project.
    ///
    /// # Errors
    ///
    /// [`PathRefusal::Outside`] for a path that is not a plain one below `designs/`,
    /// [`PathRefusal::BadType`] for one that names no PNG, JPEG or WebP file.
    pub fn parse(text: &str) -> Result<Self, PathRefusal> {
        if text.chars().count() > MAX_PATH_CHARS {
            return Err(PathRefusal::Outside);
        }
        let relative = text.strip_prefix(".rbrowser/").unwrap_or(text);
        let mut segments = relative.split('/');
        if segments.next() != Some(DESIGNS_DIR) {
            return Err(PathRefusal::Outside);
        }
        let rest: Vec<&str> = segments.collect();
        if rest.is_empty() || !rest.iter().all(|segment| plain(segment)) {
            return Err(PathRefusal::Outside);
        }
        let kind = Kind::of(relative).ok_or(PathRefusal::BadType)?;
        Ok(Self {
            path: relative.to_owned(),
            kind,
        })
    }

    /// The design of a name `project.list` gave for `.rbrowser/designs/`; none for a folder
    /// (`sub/`) or a name that is no design.
    #[must_use]
    pub fn listed(name: &str) -> Option<Self> {
        if name.ends_with('/') {
            return None;
        }
        Self::parse(&format!("{DESIGNS_DIR}/{name}")).ok()
    }

    /// The path relative to `.rbrowser/` (`designs/mobile.png`): what settings and answers name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// The path relative to the project, for `project.read`.
    #[must_use]
    pub fn project_path(&self) -> String {
        format!(".rbrowser/{}", self.path)
    }

    /// The image type its extension names.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }
}

/// Whether `id` can be a breakpoint id: 1 to 64 of `[A-Za-z0-9_-]`.
#[must_use]
pub fn breakpoint_id_ok(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
