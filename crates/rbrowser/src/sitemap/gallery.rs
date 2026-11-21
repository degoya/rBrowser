//! The gallery of a sitemap capture (RB-090-83): the command `sitemap.gallery` (`Leader C I`) -
//! the sitemap bar's button and the menu run it - opens a run's gallery (the latest run's
//! without a `runId`) where the person's `sitemap.galleryOpensIn` says: a window of rBrowser's
//! own (`cef_app/gallery_window.rs`, one window, shown again for the next gallery) or the
//! system's browser, within `app.open_link`'s budget. The MCP tool `sitemap_gallery` opens the
//! window only: an agent never hands a file of the repository to the system browser, where it
//! would run with script (security review, wave 090-w21). Never in the grid's views.
//!
//! The window shows the gallery page of the shell's pages (RB-090-86, [`GalleryOpen::url`],
//! [`super::gallery_page`]): Nuxt UI over the run's `run.json`, its images on the window's own
//! route. The system browser gets the plugin's `index.html`, the portable export, as before.
//!
//! The window opens a running run too (RB-090-88): its page reads `run.json`, which the run keeps
//! writing, and reads it again every few seconds. The `index.html` is written when the run ends;
//! the person's system browser without it gets a notice that it comes at the end
//! ([`at_end_notice`]) instead of an error.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rb_config::GalleryOpensIn;
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use rb_visual::sitemap_store::{GALLERY_FILE, RunStore};
use serde::{Deserialize, Serialize};

use super::status::RunError;
use super::tool::RunArgs;
use crate::control::Core;

/// The window's title; `{runId}`.
pub const TITLE_KEY: &str = "sitemap.gallery.title";
/// The notice of a gallery for the system browser before its `index.html` is written; `{runId}`.
pub const AT_END_KEY: &str = "sitemap.notice.galleryAtEnd";
/// The setting the notice links to: the window shows a running run.
const OPENS_IN_KEY: &str = "sitemap.galleryOpensIn";

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
    /// The run; the window's image route serves its shots alone.
    pub run_id: String,
    /// The gallery page: `rbrowser://ui/settings.html#/sitemap-gallery?run=<id>`.
    pub url: String,
    /// The window's title, translated.
    pub title: String,
}

/// Counts the galleries opened: each gets an address of its own, so the page reads its run again
/// when the window opens the same run once more (a new hash, not the same document).
static OPENED: AtomicU64 = AtomicU64::new(0);

impl GalleryOpen {
    /// The gallery page of run `run_id`, titled `title`; `None` for an id that is no plain name.
    #[must_use]
    pub fn new(run_id: &str, title: String) -> Option<Self> {
        rb_visual::slug::valid_name(run_id).then(|| {
            let opened = OPENED.fetch_add(1, Ordering::Relaxed);
            Self {
                run_id: run_id.to_owned(),
                url: crate::shell::page_url(&format!(
                    "{}?run={run_id}&open={opened}",
                    super::gallery_page::ROUTE
                )),
                title,
            }
        })
    }
}

/// What `sitemap_gallery` and `sitemap.gallery` answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryAnswer {
    pub run_id: String,
    /// The gallery file `index.html`; `None` (`null`) until the run has ended and written it.
    #[serde(default)]
    pub gallery: Option<String>,
    pub opened_in: GalleryOpensIn,
}

/// The gallery file of run `run_id` below `root` for a gallery that opens in `opened_in`: its
/// `index.html` once written. The window needs none - it reads `run.json`, a running run's too -
/// the system browser does.
///
/// # Errors
///
/// No such run, a folder that cannot be read, or for the system browser a run without its
/// `index.html` yet ([`RunError::NoGallery`]).
pub fn gallery_of(
    root: &Path,
    run_id: &str,
    opened_in: GalleryOpensIn,
) -> Result<Option<PathBuf>, RunError> {
    let store = RunStore::open(root, run_id, false)?
        .ok_or_else(|| RunError::NotFound(run_id.to_owned()))?;
    let file = store.dir().join(GALLERY_FILE);
    let written = std::fs::symlink_metadata(&file).is_ok_and(|metadata| metadata.is_file());
    match (written, opened_in) {
        (true, _) => Ok(Some(file)),
        (false, GalleryOpensIn::Window) => Ok(None),
        (false, GalleryOpensIn::System) => Err(RunError::NoGallery(run_id.to_owned())),
    }
}

/// The notice for the person whose system browser has no `index.html` of run `run_id` to open
/// yet: it is written when the run ends; the link opens the setting that picks the window.
#[must_use]
pub fn at_end_notice(run_id: &str) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Info,
        message_key: AT_END_KEY.to_owned(),
        args: [("runId".to_owned(), run_id.to_owned())].into(),
        settings_key: Some(OPENS_IN_KEY.to_owned()),
    })
}

/// Opens the gallery of `args.run_id` (the latest run's without) for `caller` as [`opens_in`]
/// says; returns at once (the window is made on its UI thread, the system browser called on a
/// blocking thread). The window opens a running run as well.
///
/// # Errors
///
/// No project, no such run, for the system browser a run without its `index.html` yet, or more
/// links opened in the system browser just now than `app.open_link` allows.
pub fn open(core: &Core, args: &RunArgs, caller: Caller) -> Result<GalleryAnswer, RunError> {
    let host = core.host();
    let root = host.project_root().ok_or(RunError::NoProject)?;
    let run_id = super::app::runs().run_id_or_latest(&root, args.run_id.as_deref())?;
    let opened_in = opens_in(core.user_settings().sitemap.gallery_opens_in, caller);
    let file = gallery_of(&root, &run_id, opened_in)?;
    match (opened_in, &file) {
        (GalleryOpensIn::Window, _) => {
            let title = rb_i18n::t_with(TITLE_KEY, &[("runId", run_id.as_str())]);
            let shown = GalleryOpen::new(&run_id, title)
                .ok_or_else(|| RunError::NotFound(run_id.clone()))?;
            core.ui().open_gallery(shown);
        }
        (GalleryOpensIn::System, Some(file)) => {
            let url = url::Url::from_file_path(file)
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
        // `gallery_of` refuses the system browser without the file.
        (GalleryOpensIn::System, None) => return Err(RunError::NoGallery(run_id)),
    }
    Ok(GalleryAnswer {
        run_id,
        gallery: file.map(|file| file.display().to_string()),
        opened_in,
    })
}

#[cfg(test)]
#[path = "gallery_tests.rs"]
mod tests;
