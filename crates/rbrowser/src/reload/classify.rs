//! What a batch of changes does to the views: CSS and images are swapped in place (JS state,
//! scroll position and form values stay), anything else reloads every view.
//!
//! Stylesheets and images are matched in the page by file name: a project path
//! (`src/styles/app.css`) and the URL it is served at (`/assets/app.css?v=3`) rarely share more.

use rb_wsl::watch::batch::Batch;
use rb_wsl::watch::filter::is_ignore_file;
use rb_wsl::watch::{Change, ChangeKind};

/// The image types swapped without a reload.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "svg", "webp", "avif", "gif"];

/// `reload.mode`: what a change on disk does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HotMode {
    /// CSS and images swapped, everything else reloaded.
    #[default]
    Auto,
    /// Every change reloads.
    Full,
    /// Changes do nothing; only the commands reload.
    Off,
}

/// The kind of a changed file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// A stylesheet.
    Css,
    /// An image.
    Image,
    /// Anything else.
    Other,
}

/// What to do in the views.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Nothing.
    Nothing,
    /// Swap these stylesheets and images (file names, each once) without a reload.
    Swap {
        /// Stylesheet file names.
        css: Vec<String>,
        /// Image file names.
        images: Vec<String>,
    },
    /// Reload.
    Full,
}

/// The kind of `path` by its extension.
#[must_use]
pub fn file_kind(path: &str) -> FileKind {
    let name = file_name(path);
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return FileKind::Other;
    };
    if stem.is_empty() {
        return FileKind::Other;
    }
    let extension = extension.to_ascii_lowercase();
    if extension == "css" {
        FileKind::Css
    } else if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        FileKind::Image
    } else {
        FileKind::Other
    }
}

/// What `batch` does under `mode`. The ignore file itself is no change of the page
/// ([`rules_changed`]).
#[must_use]
pub fn plan(batch: &Batch, mode: HotMode) -> Plan {
    if mode == HotMode::Off {
        return Plan::Nothing;
    }
    let Batch::Changes(changes) = batch else {
        return Plan::Full;
    };
    let changes: Vec<&Change> = changes
        .iter()
        .filter(|change| !is_ignore_file(&change.path))
        .collect();
    if changes.is_empty() {
        return Plan::Nothing;
    }
    if mode == HotMode::Full {
        return Plan::Full;
    }
    let (mut css, mut images) = (Vec::new(), Vec::new());
    for change in changes {
        // A stylesheet or image that is gone, or has another name, is still linked by the page
        // under the old one: only a reload makes that right.
        let kind = match change.kind {
            ChangeKind::Remove | ChangeKind::Rename => FileKind::Other,
            ChangeKind::Create | ChangeKind::Modify => file_kind(&change.path),
        };
        let list = match kind {
            FileKind::Css => &mut css,
            FileKind::Image => &mut images,
            FileKind::Other => return Plan::Full,
        };
        let name = file_name(&change.path).to_owned();
        if !list.contains(&name) {
            list.push(name);
        }
    }
    Plan::Swap { css, images }
}

/// Whether `batch` changed the project's ignore file, so the watch needs new rules.
#[must_use]
pub fn rules_changed(batch: &Batch) -> bool {
    matches!(batch, Batch::Changes(changes) if changes.iter().any(|c| is_ignore_file(&c.path)))
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
