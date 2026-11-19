//! The gallery of a sitemap capture (RB-090-83): the command `sitemap.gallery` (`Leader C I`) -
//! the sitemap bar's button and the menu run it - opens a run's `index.html` (the latest run's
//! without a `runId`) where the person's `sitemap.galleryOpensIn` says: a window of rBrowser's
//! own (`cef_app/gallery_window.rs`, one window, shown again for the next gallery) or the
//! system's browser, within `app.open_link`'s budget. The MCP tool `sitemap_gallery` opens the
//! window only: an agent never hands a file of the repository to the system browser, where it
//! would run with script (security review, wave 090-w21). Never in the grid's views.
//!
//! The window's main frame loads only what lies in the run's folder ([`GalleryOpen::may_load`]);
//! a link the person clicks opens through `app.open_link`, and JavaScript is off there - the
//! gallery has none (its own policy allows no script).

use std::path::{Path, PathBuf};

use rb_config::GalleryOpensIn;
use rb_visual::sitemap_store::{GALLERY_FILE, RunStore};
use serde::{Deserialize, Serialize};

use super::status::RunError;
use super::tool::RunArgs;
use crate::compare::app::file_url;
use crate::control::Core;

/// The window's title; `{runId}`.
pub const TITLE_KEY: &str = "sitemap.gallery.title";

/// Who asks for a gallery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caller {
    /// The person: the command, the bar, the menu.
    Person,
    /// An agent over MCP.
    Agent,
}

/// Where `caller`'s gallery opens with the setting `setting`: an agent's always in the window.
#[must_use]
pub fn opens_in(setting: GalleryOpensIn, caller: Caller) -> GalleryOpensIn {
    match caller {
        Caller::Person => setting,
        Caller::Agent => GalleryOpensIn::Window,
    }
}

/// What the gallery window shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GalleryOpen {
    pub run_id: String,
    /// The gallery's `file:` address.
    pub url: String,
    /// The run folder's `file:` address with its closing `/`: nothing outside it loads.
    pub folder: String,
    /// The window's title, translated.
    pub title: String,
}

impl GalleryOpen {
    /// The gallery `file` of run `run_id`, titled `title`.
    #[must_use]
    pub fn new(run_id: &str, file: &Path, title: String) -> Self {
        let url = file_url(&file.display().to_string());
        let folder = match url.rfind('/') {
            Some(at) => url[..=at].to_owned(),
            None => url.clone(),
        };
        Self {
            run_id: run_id.to_owned(),
            url,
            folder,
            title,
        }
    }

    /// Whether the window's main frame may load `url`: a `file:` address below the run's
    /// folder, compared without case and escapes (Chromium spells them its own way), with no
    /// `..` on the way.
    #[must_use]
    pub fn may_load(&self, url: &str) -> bool {
        let address = url.split(['?', '#']).next().unwrap_or_default();
        let address = unescaped(address).to_lowercase();
        let folder = unescaped(&self.folder).to_lowercase();
        address
            .strip_prefix(&folder)
            .is_some_and(|rest| !rest.split(['/', '\\']).any(|segment| segment == ".."))
    }
}

/// `text` with its `%XX` escapes decoded (an escape that is none stays as it is).
fn unescaped(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = match bytes.get(at..at + 3) {
            Some([b'%', high, low]) if high.is_ascii_hexdigit() && low.is_ascii_hexdigit() => {
                std::str::from_utf8(&bytes[at + 1..at + 3]).ok()
            }
            _ => None,
        };
        let escape = hex.and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escape {
            Some(byte) => {
                out.push(byte);
                at += 3;
            }
            None => {
                out.push(bytes[at]);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What `sitemap_gallery` and `sitemap.gallery` answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryAnswer {
    pub run_id: String,
    /// The gallery file.
    pub gallery: String,
    pub opened_in: GalleryOpensIn,
}

/// The gallery of run `run_id` below `root`: its `index.html`, a file and no link.
///
/// # Errors
///
/// No such run, a run without a gallery yet, or a folder that cannot be read.
pub fn gallery_file(root: &Path, run_id: &str) -> Result<PathBuf, RunError> {
    let store = RunStore::open(root, run_id, false)?
        .ok_or_else(|| RunError::NotFound(run_id.to_owned()))?;
    let file = store.dir().join(GALLERY_FILE);
    match std::fs::symlink_metadata(&file) {
        Ok(metadata) if metadata.is_file() => Ok(file),
        _ => Err(RunError::NoGallery(run_id.to_owned())),
    }
}

/// Opens the gallery of `args.run_id` (the latest run's without) for `caller` as [`opens_in`]
/// says; returns at once (the window is made on its UI thread, the system browser called on a
/// blocking thread).
///
/// # Errors
///
/// No project, no such run, a run without a gallery yet, or more links opened in the system
/// browser just now than `app.open_link` allows.
pub fn open(core: &Core, args: &RunArgs, caller: Caller) -> Result<GalleryAnswer, RunError> {
    let host = core.host();
    let root = host.project_root().ok_or(RunError::NoProject)?;
    let run_id = super::app::runs().run_id_or_latest(&root, args.run_id.as_deref())?;
    let file = gallery_file(&root, &run_id)?;
    let opened_in = opens_in(core.user_settings().sitemap.gallery_opens_in, caller);
    match opened_in {
        GalleryOpensIn::Window => {
            let title = rb_i18n::t_with(TITLE_KEY, &[("runId", run_id.as_str())]);
            core.ui()
                .open_gallery(GalleryOpen::new(&run_id, &file, title));
        }
        GalleryOpensIn::System => {
            let url = url::Url::from_file_path(&file)
                .map_err(|()| RunError::Failed("the gallery has no file address".to_owned()))?;
            if !crate::open_link::take_slot() {
                return Err(RunError::Failed(
                    "more links were opened just now than allowed; try again in a moment"
                        .to_owned(),
                ));
            }
            core.runtime().spawn_blocking(move || {
                if let Err(error) = crate::open_link::system::open(&url) {
                    tracing::warn!(%error, "the gallery did not open in the system browser");
                }
            });
        }
    }
    Ok(GalleryAnswer {
        run_id,
        gallery: file.display().to_string(),
        opened_in,
    })
}

#[cfg(test)]
#[path = "gallery_tests.rs"]
mod tests;
