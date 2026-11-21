//! The gallery page of the app's gallery window (RB-090-86): a page of the shell's pages
//! (`ui/src/sitemap/gallery/`, the route `#/sitemap-gallery?run=<id>`) shows a run with Nuxt UI
//! instead of the plugin's `index.html`, which stays the portable export for the system browser.
//!
//! - **The run** comes through the query `getSitemapGallery` ([`ipc`]): `run.json` read by
//!   `rb_visual::sitemap_store` (no link followed, plain names) and turned into the page's
//!   shape ([`page_of`]); a shot's image is an address on the gallery window's route, never a
//!   path.
//! - **The images** are `rbrowser://sitemap/<run>/<slug>/<breakpoint>.png` ([`shot_url`]):
//!   `cef_app/sitemap_scheme.rs` answers them for the gallery window's main frame alone, and
//!   [`open_shot`] opens only a shot of the run that window shows - three plain names, the
//!   `.png` of a page's folder, walked without a link by [`RunStore::open_shot`], at most
//!   [`MAX_GALLERY_SHOT_BYTES`] - and the route streams it in chunks. No listing,
//!   no `run.json`, no gallery file, nothing of another run or outside the run's folder.
//! - **The thumbnails** are as wide as the plugin's setting `sitemap-capture.thumbnailWidth`,
//!   held to the plugin's own settings schema ([`thumbnail_width`]).
//! - **The query** runs off the UI thread ([`dispatch_off_thread`]) and carries at most
//!   [`MAX_PAGES`] pages.

use std::path::Path;

use rb_ipc::{
    CoreToShell, GalleryBreakpoint, GalleryCompare, GalleryPage, GalleryShot, IpcErrorCode,
    ShellToCore, SitemapGalleryPage,
};
use rb_visual::sitemap::{RunState, ShotState, SitemapRun};
use rb_visual::sitemap_store::{RunStore, ShotFile};
use rb_visual::slug::valid_name;
use serde_json::Value;

use super::status::RunError;
use crate::control::Core;
use crate::ipc::Failure;

/// The host of the gallery window's image route: `rbrowser://sitemap/...`.
pub const ROUTE_HOST: &str = "sitemap";
/// The page's route in the shell's pages.
pub const ROUTE: &str = "sitemap-gallery";
/// The plugin setting of the thumbnail width.
pub const THUMBNAIL_SETTING: &str = "sitemap-capture.thumbnailWidth";
/// The plugin's settings schema as bundled: the one source of the width's choices and default,
/// for when the plugin is not loaded (a loaded plugin's own schema comes first).
pub const BUNDLED_SCHEMA: &str =
    include_str!("../../../../plugins/sitemap-capture/settings.schema.json");
/// The most pages one answer carries; the run's `index.html` has them all.
pub const MAX_PAGES: usize = 2000;

/// The run, page and breakpoint a shot address names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShotAddress {
    pub run_id: String,
    pub slug: String,
    pub breakpoint: String,
}

/// Why a shot was not served; [`ShotRefusal::code`] goes to the log, never a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotRefusal {
    /// The address names no shot.
    Address,
    /// The shot is not of the run the gallery window shows.
    OtherRun,
    /// No project is open.
    NoProject,
    /// No such run or shot.
    Missing,
    /// A link on the way, a shot too large, or a read that failed.
    Unreadable,
}

impl ShotRefusal {
    /// The stable code for the log.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Address => "address",
            Self::OtherRun => "other-run",
            Self::NoProject => "no-project",
            Self::Missing => "missing",
            Self::Unreadable => "unreadable",
        }
    }
}

/// The address of the shot of `breakpoint` on the page `slug` of run `run_id`; `None` unless
/// all three are plain names.
#[must_use]
pub fn shot_url(run_id: &str, slug: &str, breakpoint: &str) -> Option<String> {
    [run_id, slug, breakpoint]
        .iter()
        .all(|name| valid_name(name))
        .then(|| format!("rbrowser://{ROUTE_HOST}/{run_id}/{slug}/{breakpoint}.png"))
}

/// The shot `url` names: exactly `rbrowser://sitemap/<run>/<slug>/<breakpoint>.png`, three plain
/// names, no query, no fragment, no escape; anything else is none.
#[must_use]
pub fn parse_shot_url(url: &str) -> Option<ShotAddress> {
    let prefix = format!("rbrowser://{ROUTE_HOST}/");
    let head = url.get(..prefix.len())?;
    if !head.eq_ignore_ascii_case(&prefix) {
        return None;
    }
    let rest = &url[prefix.len()..];
    if rest.contains(['?', '#', '%', '\\']) {
        return None;
    }
    let mut parts = rest.split('/');
    let (Some(run_id), Some(slug), Some(file), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let breakpoint = file.strip_suffix(".png")?;
    [run_id, slug, breakpoint]
        .iter()
        .all(|name| valid_name(name))
        .then(|| ShotAddress {
            run_id: run_id.to_owned(),
            slug: slug.to_owned(),
            breakpoint: breakpoint.to_owned(),
        })
}

/// Whether a shot request comes from the gallery window's main frame: `gallery` is its browser
/// id (0 while it is closed), `browser` the one that asks.
#[must_use]
pub fn requester_allowed(gallery: i32, browser: i32, main_frame: bool) -> bool {
    gallery != 0 && browser == gallery && main_frame
}

/// The longest shot the gallery window gets: a full-page PNG of a long page fits, and a
/// thumbnail of a foreign repository cannot ask for more.
pub const MAX_GALLERY_SHOT_BYTES: u64 = 16 * 1024 * 1024;

/// The shot `url` names, opened below the project `root` when it is of the run `shown` - the
/// one the gallery window shows - and at most [`MAX_GALLERY_SHOT_BYTES`] long; the caller streams
/// it.
///
/// # Errors
///
/// The [`ShotRefusal`].
pub fn open_shot(
    root: Option<&Path>,
    shown: Option<&str>,
    url: &str,
) -> Result<ShotFile, ShotRefusal> {
    let address = parse_shot_url(url).ok_or(ShotRefusal::Address)?;
    if shown != Some(address.run_id.as_str()) {
        return Err(ShotRefusal::OtherRun);
    }
    let root = root.ok_or(ShotRefusal::NoProject)?;
    let store = RunStore::open(root, &address.run_id, false)
        .map_err(|_| ShotRefusal::Unreadable)?
        .ok_or(ShotRefusal::Missing)?;
    store
        .open_shot(&address.slug, &address.breakpoint, MAX_GALLERY_SHOT_BYTES)
        .map_err(|_| ShotRefusal::Unreadable)?
        .ok_or(ShotRefusal::Missing)
}

/// The choices and the default of `thumbnailWidth` in the plugin's settings `schema`.
fn width_rule(schema: &Value) -> Option<(Vec<u64>, u64)> {
    let key = schema.get("properties")?.get("thumbnailWidth")?;
    let choices: Vec<u64> = key
        .get("enum")?
        .as_array()?
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    let default = key.get("default")?.as_u64()?;
    choices.contains(&default).then_some((choices, default))
}

/// The thumbnail width of the setting's `value`, held to the plugin's settings schema - the
/// `loaded` plugin's, else the bundled one ([`BUNDLED_SCHEMA`]): one of its choices, else its
/// default.
#[must_use]
pub fn thumbnail_width(value: Option<&Value>, loaded: Option<&Value>) -> u32 {
    let bundled = || {
        serde_json::from_str::<Value>(BUNDLED_SCHEMA)
            .ok()
            .and_then(|schema| width_rule(&schema))
    };
    let (choices, default) = loaded.and_then(width_rule).or_else(bundled).expect(
        "the bundled schema names thumbnailWidth's choices and default (gallery_page_tests)",
    );
    let width = value
        .and_then(Value::as_u64)
        .filter(|width| choices.contains(width))
        .unwrap_or(default);
    u32::try_from(width).unwrap_or(u32::MAX)
}

fn run_state(state: RunState) -> &'static str {
    match state {
        RunState::Running => "running",
        RunState::Cancelled => "cancelled",
        RunState::Finished => "finished",
    }
}

fn shot_state(state: ShotState) -> &'static str {
    match state {
        ShotState::Pending => "pending",
        ShotState::Done => "done",
        ShotState::Failed => "failed",
    }
}

/// `run` as the gallery page shows it, its thumbnails `thumbnail_width` CSS px wide: its first
/// [`MAX_PAGES`] pages, and how many there are when that is fewer. A breakpoint whose name is not
/// plain is left out; a shot gets its image only when it is done and its page's slug is a plain
/// name.
#[must_use]
pub fn page_of(run: &SitemapRun, thumbnail_width: u32) -> SitemapGalleryPage {
    let breakpoints: Vec<GalleryBreakpoint> = run
        .breakpoints
        .iter()
        .filter(|breakpoint| valid_name(&breakpoint.name))
        .map(|breakpoint| GalleryBreakpoint {
            name: breakpoint.name.clone(),
            label: breakpoint.label.clone(),
            width: breakpoint.width,
            height: breakpoint.height,
        })
        .collect();
    let pages = run
        .pages
        .iter()
        .take(MAX_PAGES)
        .map(|page| GalleryPage {
            url: page.url.clone(),
            status: page.status,
            final_url: page.final_url.clone(),
            shots: page
                .shots
                .iter()
                .filter(|(name, _)| breakpoints.iter().any(|known| &known.name == *name))
                .map(|(name, shot)| {
                    let image = (shot.state == ShotState::Done)
                        .then(|| shot_url(&run.run_id, &page.slug, name))
                        .flatten();
                    let entry = GalleryShot {
                        state: shot_state(shot.state).to_owned(),
                        error: shot.error.clone(),
                        image,
                        compare: shot.compare.as_ref().map(|compare| GalleryCompare {
                            status: compare.status.clone(),
                            ratio: compare.ratio,
                        }),
                    };
                    (name.clone(), entry)
                })
                .collect(),
        })
        .collect();
    SitemapGalleryPage {
        run_id: run.run_id.clone(),
        source: run.source.clone(),
        state: run_state(run.state).to_owned(),
        thumbnail_width,
        breakpoints,
        pages,
        total_pages: (run.pages.len() > MAX_PAGES)
            .then(|| u32::try_from(run.pages.len()).unwrap_or(u32::MAX)),
    }
}

/// The gallery page of run `run_id` below `root`.
///
/// # Errors
///
/// No such run (or no `run.json` yet), an id that is no plain name, or a folder or `run.json`
/// that cannot be read.
pub fn load(
    root: &Path,
    run_id: &str,
    thumbnail_width: u32,
) -> Result<SitemapGalleryPage, RunError> {
    let store = RunStore::open(root, run_id, false)?
        .ok_or_else(|| RunError::NotFound(run_id.to_owned()))?;
    let run = store
        .load()?
        .ok_or_else(|| RunError::NotFound(run_id.to_owned()))?;
    Ok(page_of(&run, thumbnail_width))
}

/// Whether `message` is the gallery page's request: [`dispatch_off_thread`] answers it.
#[must_use]
pub fn is_gallery_request(message: &ShellToCore) -> bool {
    matches!(message, ShellToCore::GetSitemapGallery(_))
}

/// Answers `getSitemapGallery` on a blocking thread of the core's runtime: `run.json` may be
/// large, and the CEF UI thread never waits for the disk (security review, wave 090-w22).
pub fn dispatch_off_thread(
    core: &Core,
    message: ShellToCore,
    reply: impl FnOnce(Result<CoreToShell, Failure>) + Send + 'static,
) {
    let Some(core) = core.arc() else {
        reply(Err((
            IpcErrorCode::Internal,
            "the window is closed".to_owned(),
        )));
        return;
    };
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        reply(ipc(&core, &message).unwrap_or_else(|| {
            Err((
                IpcErrorCode::BadRequest,
                "no sitemap gallery request".to_owned(),
            ))
        }));
    });
}

/// Answers `getSitemapGallery`: the named run, else the latest; `None` for any other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let ShellToCore::GetSitemapGallery(request) = message else {
        return None;
    };
    let answer = core
        .host()
        .project_root()
        .ok_or(RunError::NoProject)
        .and_then(|root| {
            let run_id = super::app::runs().run_id_or_latest(&root, request.run_id.as_deref())?;
            let services = crate::plugins::services::shared();
            let values = services.plugin_values(super::app::PLUGIN_ID);
            let schema = services.plugin_schema(super::app::PLUGIN_ID);
            let width = thumbnail_width(values.get(THUMBNAIL_SETTING), schema.as_ref());
            load(&root, &run_id, width)
        });
    Some(answer.map(CoreToShell::SitemapGallery).map_err(|error| {
        let code = match error {
            RunError::NotFound(_) | RunError::NoProject => IpcErrorCode::NotFound,
            RunError::Store(_) => IpcErrorCode::Refused,
            _ => IpcErrorCode::Internal,
        };
        // The message starts with the code (an i18n key) the page shows.
        (code, error.to_string())
    }))
}

#[cfg(test)]
#[path = "gallery_page_tests.rs"]
mod tests;
