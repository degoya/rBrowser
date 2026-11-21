//! The rules of the last pages (RB-092-04), without a runtime: what a view keeps of its page,
//! how the file reads back, which view an entry belongs to and where it leads now.
//!
//! An entry names its view by the breakpoint of its cell and the count of that breakpoint's
//! views before it in grid order, so it finds the same cell after a restart (view ids are not
//! kept). A page under the environment in effect is kept as its path (as a bookmark,
//! `rb_config::bookmark_paths::path_in`), any other web page as its URL; a URL with user info
//! is never kept.

use rb_config::bookmarks::{base_url, is_bookmark_path};
use rb_config::{ProjectConfig, StartupRestore};
use rb_engine::ViewId;
use rb_util::url::is_web_url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The version of the file's format.
pub const FORMAT_VERSION: u32 = 1;
/// At most this many entries are kept or read.
pub const MAX_ENTRIES: usize = 64;
/// A longer URL is not kept.
pub const MAX_URL_LEN: usize = 8192;
/// A longer breakpoint id is no entry.
const MAX_BREAKPOINT_LEN: usize = 128;

/// The pages of the views when they were last kept.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastPages {
    /// [`FORMAT_VERSION`].
    pub version: u32,
    /// One entry per view that had something to keep, in grid order.
    #[serde(default)]
    pub views: Vec<LastPage>,
}

/// What one view kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastPage {
    /// The breakpoint of the view's cell.
    pub breakpoint: String,
    /// How many views of that breakpoint came before it in grid order.
    #[serde(default)]
    pub index: u32,
    /// The page as a path under the environment in effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The page as an absolute `http`/`https` URL, when it lay under no environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The view's navigation was decoupled (RB-020-06).
    #[serde(default, skip_serializing_if = "is_false")]
    pub decoupled: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// One view as [`capture`] reads it.
#[derive(Clone, Copy, Debug)]
pub struct ViewSnapshot<'a> {
    /// The breakpoint of its cell; `None` for a view outside the grid's cells.
    pub cell_breakpoint: Option<&'a str>,
    /// Its current URL.
    pub url: &'a str,
    /// Its navigation is decoupled.
    pub decoupled: bool,
}

/// What [`resolve`] gives one view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restore {
    /// The view.
    pub view: ViewId,
    /// The page it opens; `None` when only its coupling comes back.
    pub url: Option<String>,
    /// It is decoupled.
    pub decoupled: bool,
}

/// Whether the views open the last pages at the start: `startup.restore = last` and no URL on
/// the command line, which always wins.
pub fn restores_at_start(setting: StartupRestore, url_on_command_line: bool) -> bool {
    setting == StartupRestore::Last && !url_on_command_line
}

/// What `views` (in grid order) keep while `environment` is in effect.
pub fn capture(
    views: &[ViewSnapshot<'_>],
    config: &ProjectConfig,
    environment: Option<&str>,
) -> LastPages {
    capture_over(views, config, environment, None)
}

/// [`capture`] over the entries kept `before`: a view that shows no page to keep now (a blank
/// page, an error page, a page still loading) keeps its entry from before for the same
/// breakpoint and index - with its coupling of now -, so a dev server that is down or a close
/// while the views load loses nothing. Entries of views that are gone are not carried over.
pub fn capture_over(
    views: &[ViewSnapshot<'_>],
    config: &ProjectConfig,
    environment: Option<&str>,
    before: Option<&LastPages>,
) -> LastPages {
    let mut seen: Vec<(&str, u32)> = Vec::new();
    let mut pages = Vec::new();
    for view in views {
        let Some(breakpoint) = view.cell_breakpoint else {
            continue;
        };
        let index = next_index(&mut seen, breakpoint);
        let (path, url) = if is_storable(view.url) {
            match rb_config::bookmark_paths::path_in(config, environment, view.url) {
                Some(path) => (Some(path), None),
                None => (None, Some(view.url.to_owned())),
            }
        } else {
            before
                .and_then(|before| {
                    before.views.iter().find(|page| {
                        page.breakpoint == breakpoint
                            && page.index == index
                            && (page.path.is_some() || page.url.is_some())
                    })
                })
                .map_or((None, None), |page| (page.path.clone(), page.url.clone()))
        };
        let page = LastPage {
            breakpoint: breakpoint.to_owned(),
            index,
            path,
            url,
            decoupled: view.decoupled,
        };
        if is_valid(&page) && pages.len() < MAX_ENTRIES {
            pages.push(page);
        }
    }
    LastPages {
        version: FORMAT_VERSION,
        views: pages,
    }
}

/// The count of `breakpoint`'s views seen so far, then one more.
fn next_index<'a>(seen: &mut Vec<(&'a str, u32)>, breakpoint: &'a str) -> u32 {
    if let Some((_, count)) = seen.iter_mut().find(|(id, _)| *id == breakpoint) {
        let index = *count;
        *count = count.saturating_add(1);
        return index;
    }
    seen.push((breakpoint, 1));
    0
}

/// Where each of `views` (in grid order, with their cell's breakpoint) goes back to while
/// `environment` is in effect. An entry without a view (its breakpoint removed, fewer views of
/// it) is dropped, and so is a path with no environment or start URL to lie under.
pub fn resolve(
    pages: &LastPages,
    views: &[(ViewId, Option<&str>)],
    config: &ProjectConfig,
    environment: Option<&str>,
) -> Vec<Restore> {
    let base = base_url(config, environment);
    let mut seen: Vec<(&str, u32)> = Vec::new();
    let mut restores = Vec::new();
    for (view, breakpoint) in views {
        let Some(breakpoint) = *breakpoint else {
            continue;
        };
        let index = next_index(&mut seen, breakpoint);
        let Some(page) = pages
            .views
            .iter()
            .find(|page| page.breakpoint == breakpoint && page.index == index)
        else {
            continue;
        };
        let url = match (&page.path, &page.url) {
            (Some(path), _) => base
                .as_deref()
                .map(|base| rb_config::bookmark_paths::url_in(base, path)),
            (None, Some(url)) => Some(url.clone()),
            (None, None) => None,
        };
        if url.is_none() && !page.decoupled {
            continue;
        }
        restores.push(Restore {
            view: *view,
            url,
            decoupled: page.decoupled,
        });
    }
    restores
}

/// The file's JSON.
pub fn to_value(pages: &LastPages) -> Value {
    serde_json::to_value(pages).unwrap_or(Value::Null)
}

/// The pages of the file's `text`; `None` when it is not this format. Entries the check
/// refuses are dropped, at most [`MAX_ENTRIES`] are read.
pub fn parse(text: &str) -> Option<LastPages> {
    let mut pages: LastPages = serde_json::from_str(text).ok()?;
    if pages.version != FORMAT_VERSION {
        return None;
    }
    pages.views.retain(is_valid);
    pages.views.truncate(MAX_ENTRIES);
    Some(pages)
}

/// Whether `url` may be kept: an `http`/`https` URL no longer than [`MAX_URL_LEN`] and without
/// user info.
pub fn is_storable(url: &str) -> bool {
    url.len() <= MAX_URL_LEN && is_web_url(url) && !has_user_info(url)
}

/// Whether the authority of `url` names a user (`https://user:password@host/`).
fn has_user_info(url: &str) -> bool {
    url.split_once("://")
        .and_then(|(_, rest)| rest.split(['/', '?', '#']).next())
        .is_some_and(|authority| authority.contains('@'))
}

/// An entry names a breakpoint and holds a path, a URL or its decoupling - never both a path
/// and a URL, and only what [`capture`] could have written.
fn is_valid(page: &LastPage) -> bool {
    let breakpoint = !page.breakpoint.is_empty()
        && page.breakpoint.len() <= MAX_BREAKPOINT_LEN
        && !page.breakpoint.chars().any(char::is_control);
    let location = match (&page.path, &page.url) {
        (Some(path), None) => path.len() <= MAX_URL_LEN && is_bookmark_path(path),
        (None, Some(url)) => is_storable(url),
        (None, None) => page.decoupled,
        (Some(_), Some(_)) => false,
    };
    breakpoint && location
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
