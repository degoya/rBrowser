//! Which images the editor opens and where its results go. It opens PNG files below
//! `.rbrowser/out/` only - never its own folder - and writes only below
//! `.rbrowser/out/screenshot-editor/` (the `out-write` capability): the edited image and the
//! annotations of `.rbrowser/out/<rel>/<name>.png` are `<rel>/<name>.edited.png` and
//! `<rel>/<name>.annotations.json` there, so the original stays untouched and opening it again
//! finds its annotations. The host checks every path once more.

use rb_plugin_api::{Failure, FailureKind};

use crate::PLUGIN_ID;

/// Every image the editor opens lies below this folder.
pub const OUT_ROOT: &str = ".rbrowser/out/";
/// The plugin's own folder, the only one it writes.
pub const OWN_ROOT: &str = ".rbrowser/out/screenshot-editor/";
/// The longest path accepted.
pub const MAX_PATH: usize = 512;

/// The format of an edited image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
}

impl Format {
    /// `png` or `jpeg`; anything else is none.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "png" => Some(Self::Png),
            "jpeg" => Some(Self::Jpeg),
            _ => None,
        }
    }

    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
        }
    }
}

/// Where one edit is written, each as a path below the plugin's own folder (`out.write`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Targets {
    pub image: String,
    pub annotations: String,
    pub format: Format,
}

impl Targets {
    /// `path` (below the plugin's folder) as the project-relative path answers name.
    #[must_use]
    pub fn project(path: &str) -> String {
        format!("{OWN_ROOT}{path}")
    }
}

fn failure(code: &str, path: &str) -> Failure {
    Failure::coded(
        FailureKind::AccessDenied,
        &format!("plugin.{PLUGIN_ID}.errors.{code}"),
    )
    .with_param("path", path.chars().take(128).collect::<String>())
}

/// `/`-separated, relative, no empty, `.` or `..` segment, no backslash, colon or control
/// character, at most [`MAX_PATH`] bytes.
fn well_formed(path: &str) -> bool {
    path.len() <= MAX_PATH
        && !path.starts_with('/')
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

/// The file name of `path` has a name before `.<extension>` (either case).
fn has_extension(path: &str, extension: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.len() > extension.len() + 1
        && name
            .get(name.len() - extension.len() - 1..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(&format!(".{extension}")))
}

/// `path` as an image the editor may open: a PNG below `.rbrowser/out/`, not the plugin's own
/// folder. `plugin.screenshot-editor.errors.path_outside` or `.not_png` otherwise.
pub fn source(path: &str) -> Result<String, Failure> {
    if !well_formed(path) || !path.starts_with(OUT_ROOT) || path.starts_with(OWN_ROOT) {
        return Err(failure("path_outside", path));
    }
    if !has_extension(path, "png") {
        return Err(failure("not_png", path));
    }
    Ok(path.to_owned())
}

/// Where the edit of `source` (checked by [`source`]) goes when the caller names no place.
#[must_use]
pub fn mirror(source: &str, format: Format) -> Targets {
    let relative = source.strip_prefix(OUT_ROOT).unwrap_or(source);
    let stem = relative
        .get(..relative.len().saturating_sub(4))
        .unwrap_or(relative);
    Targets {
        image: format!("{stem}.edited.{}", format.extension()),
        annotations: format!("{stem}.annotations.json"),
        format,
    }
}

/// The caller's `out`: a project-relative path below `.rbrowser/out/screenshot-editor/` ending in
/// `.png`, `.jpg` or `.jpeg` (which names the format); the annotations go beside it. Any other
/// place - another folder of `.rbrowser/out/`, outside it, absolute, `..` - is
/// `plugin.screenshot-editor.errors.path_outside`.
pub fn custom(out: &str) -> Result<Targets, Failure> {
    if !well_formed(out) || !out.starts_with(OWN_ROOT) {
        return Err(failure("path_outside", out));
    }
    let relative = out.strip_prefix(OWN_ROOT).unwrap_or(out);
    let (format, cut) = if has_extension(relative, "png") {
        (Format::Png, 4)
    } else if has_extension(relative, "jpg") {
        (Format::Jpeg, 4)
    } else if has_extension(relative, "jpeg") {
        (Format::Jpeg, 5)
    } else {
        return Err(failure("out_extension", out));
    };
    let stem = relative.get(..relative.len() - cut).unwrap_or(relative);
    if stem.is_empty() || stem.ends_with('/') || stem.ends_with(".annotations") {
        return Err(failure("out_extension", out));
    }
    Ok(Targets {
        image: relative.to_owned(),
        annotations: format!("{stem}.annotations.json"),
        format,
    })
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
