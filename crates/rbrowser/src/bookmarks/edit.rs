//! "Bookmark this page" as an edit of one config file (RB-070-06): the bookmark the arguments of
//! `bookmarks.add` describe - with the active view's page filling what they leave out, as its
//! path under the environments while `bookmarks.followEnvironment` is on or the dialog's
//! switch says so (RB-080-48; the rule is `rb_config::bookmark_paths`) - appended to `bookmarks[]` of the document, every other key kept. Pure; the
//! file work is `super::store`.

use rb_config::bookmarks::new_id;
use rb_config::{Bookmark, BookmarkTarget};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Which file a new bookmark goes to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AddScope {
    /// `config.json`, the team's.
    #[default]
    Project,
    /// `config.local.json`, one's own.
    Local,
}

/// The arguments of `bookmarks.add`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddArgs {
    /// The id; made from the label when absent.
    #[serde(default)]
    pub id: Option<String>,
    /// The label; the page title (else the target) when absent.
    #[serde(default)]
    pub label: Option<String>,
    /// A path after the environment's URL.
    #[serde(default)]
    pub path: Option<String>,
    /// An absolute URL.
    #[serde(default)]
    pub url: Option<String>,
    /// The group of the toolbar list.
    #[serde(default)]
    pub group: Option<String>,
    /// A direct shortcut.
    #[serde(default)]
    pub shortcut: Option<String>,
    /// Whether it follows the environment (the dialog's switch): `true` turns a `url` that has
    /// a path under the environments into that path, `false` a `path` into its URL in the
    /// environment in effect. Absent, a target is taken as given and the page of the active
    /// view follows as `bookmarks.followEnvironment` says.
    #[serde(default)]
    pub follow: Option<bool>,
    /// The file; `project` when absent.
    #[serde(default)]
    pub scope: AddScope,
}

impl AddArgs {
    /// Whether the arguments name neither label nor target: the shell's dialog asks.
    pub fn is_empty(&self) -> bool {
        self.label.is_none() && self.path.is_none() && self.url.is_none()
    }
}

/// The page the active view shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// Its URL.
    pub url: String,
    /// Its title.
    pub title: String,
}

/// What `bookmarks.add` knows of the environments: the one implementation of which form a
/// bookmark takes (`rb_config::bookmark_paths`), handed in so this stays pure.
pub struct Follow<'a> {
    /// `bookmarks.followEnvironment`: what holds when the arguments say nothing.
    pub default: bool,
    /// The path a URL is stored as, when it has one (`rb_config::bookmark_paths::path_in`).
    pub path_of: &'a dyn Fn(&str) -> Option<String>,
    /// The base in effect, which a path is put after.
    pub base: Option<&'a str>,
}

/// The bookmark `args` describe: a missing target is `page`, a missing label the page title or
/// else the target, a missing id one made from the label that none of `taken` has. Whether the
/// target is stored as a path or as a URL is `args.follow`, else for the page
/// `bookmarks.followEnvironment`; a target named without `follow` is taken as given. A URL
/// with no path under the environments stays a URL. The config check judges the forms
/// afterwards.
///
/// # Errors
///
/// Both `path` and `url`, no target and no web page to take one from, a path to store as a URL
/// with no base to put it after, or an id in `taken`.
pub fn bookmark_of(
    args: &AddArgs,
    page: Option<&Page>,
    follow: &Follow<'_>,
    taken: &[String],
) -> Result<Bookmark, String> {
    let as_path = |url: String| match (follow.path_of)(&url) {
        Some(path) => BookmarkTarget::Path(path),
        None => BookmarkTarget::Url(url),
    };
    let target = match (given(&args.path), given(&args.url)) {
        (Some(_), Some(_)) => return Err("give either `path` or `url`, not both".to_owned()),
        (Some(path), None) if args.follow == Some(false) => {
            let base = follow
                .base
                .ok_or("there is no environment or start URL to put the path after")?;
            if !rb_config::bookmarks::is_bookmark_path(&path) {
                return Err(format!("`{path}` is no path a bookmark may hold"));
            }
            BookmarkTarget::Url(rb_config::bookmark_paths::url_in(base, &path))
        }
        (Some(path), None) => BookmarkTarget::Path(path),
        (None, Some(url)) if args.follow == Some(true) => as_path(url),
        (None, Some(url)) => BookmarkTarget::Url(url),
        (None, None) => match page {
            Some(page) if rb_util::url::is_web_url(&page.url) => {
                if args.follow.unwrap_or(follow.default) {
                    as_path(page.url.clone())
                } else {
                    BookmarkTarget::Url(page.url.clone())
                }
            }
            _ => return Err("the active view shows no http or https page".to_owned()),
        },
    };
    let shown = match &target {
        BookmarkTarget::Path(text) | BookmarkTarget::Url(text) => text.clone(),
    };
    let label = given(&args.label)
        .or_else(|| page.and_then(|page| given(&Some(page.title.clone()))))
        .unwrap_or(shown);
    let id = match given(&args.id) {
        Some(id) if taken.contains(&id) => {
            return Err(format!("a bookmark `{id}` exists already"));
        }
        Some(id) => id,
        None => new_id(&label, taken),
    };
    let (path, url) = match target {
        BookmarkTarget::Path(path) => (Some(path), None),
        BookmarkTarget::Url(url) => (None, Some(url)),
    };
    Ok(Bookmark {
        id,
        label,
        path,
        url,
        group: given(&args.group),
        shortcut: given(&args.shortcut),
    })
}

/// `text` trimmed, when anything is left.
fn given(text: &Option<String>) -> Option<String> {
    text.as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// Appends `bookmark` to `bookmarks[]` of `document` (made when absent).
///
/// # Errors
///
/// The document is no object or its `bookmarks` no list.
pub fn append(document: &mut Value, bookmark: &Bookmark) -> Result<(), String> {
    let item = serde_json::to_value(bookmark).map_err(|error| error.to_string())?;
    document
        .as_object_mut()
        .ok_or_else(|| "the config file holds no object".to_owned())?
        .entry(rb_config::bookmarks::BOOKMARKS_KEY)
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| "`bookmarks` in the config file is no list".to_owned())?
        .push(item);
    Ok(())
}

/// The ids of `bookmarks[]` in `document`.
pub fn ids_in(document: &Value) -> Vec<String> {
    document
        .get(rb_config::bookmarks::BOOKMARKS_KEY)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("id").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
