//! Project bookmarks (RB-070-06): `bookmarks[]` of `config.json` (the team's) and
//! `config.local.json` (one's own, merged by `id`). A bookmark names its page by exactly one of
//! `path` - put after the URL of the environment in effect, so it follows an environment switch -
//! or `url`, absolute and left as it is.
//!
//! Bookmarks only ever navigate when a person (or an agent through `navigate`) opens one, and
//! only to `http`/`https`; they grant nothing, so like `startUrl` and `environments` they are not
//! gated by workspace trust. A `javascript:`, `file:` or `data:` URL, a relative path or a
//! bookmark with both or neither target is refused by the check ([`check_bookmarks`]: the
//! settings window and MCP cannot save it) and dropped with a warning when a file on disk holds
//! it ([`strip_invalid`]: the rest of the config stays in force).
//!
//! A `path` only ever lies under the host of its base (RB-080-48): `//host` and `/\host`, which a
//! browser reads as another host, are refused like a scheme, and [`Bookmark::url_with`] holds the
//! result to the origin of the base. Which path a page has under the environments -
//! what "bookmark this page" stores while `bookmarks.followEnvironment` is on - is
//! [`crate::bookmark_paths`].
//!
//! [`resolve`] is the list other features read (the sitemap capture of RB-050-05).

use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ConfigIssue, ProjectConfig};

/// The config key of the bookmarks.
pub const BOOKMARKS_KEY: &str = "bookmarks";
/// The i18n key of a bookmark with both or neither of `path` and `url`.
pub const TARGET_KEY: &str = "config.error.bookmark_target";
/// The i18n key of a `path` that does not start with one `/` (a scheme, `//host`, `/\host`) or
/// holds a space or control character.
pub const PATH_KEY: &str = "config.error.bookmark_path";
/// The i18n key of a `url` that is no absolute `http`/`https` URL.
pub const URL_KEY: &str = "config.error.bookmark_url";
/// The i18n key of an empty id.
pub const ID_KEY: &str = "config.error.bookmark_id";
/// The i18n key of an empty label.
pub const LABEL_KEY: &str = "config.error.bookmark_label";
/// The i18n key of the warning when loading dropped a bookmark.
pub const IGNORED_KEY: &str = "config.warning.bookmark_ignored";

/// One entry of `bookmarks[]`; `x-new` is the entry "Add" starts in the settings (RB-090-61).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("oneOf" = [{ "required": ["path"] }, { "required": ["url"] }]))]
#[schemars(extend("x-new" = { "id": "home", "label": "Home", "path": "/" }))]
pub struct Bookmark {
    /// Unique id; `config.local.json` changes or adds a bookmark by it.
    pub id: String,
    /// The name in the toolbar list and the command palette.
    pub label: String,
    /// A path such as `/products/42` (query and hash allowed), put after the URL of the
    /// environment in effect. Exactly one of `path` and `url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// An absolute `http`/`https` URL, the same in every environment. Exactly one of `path` and
    /// `url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The group the toolbar list files it under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// A direct shortcut that opens it (`Alt+F7`); held to the reserved list and to every other
    /// binding like a breakpoint's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
}

/// Where a bookmark leads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BookmarkTarget {
    /// After the environment's URL.
    Path(String),
    /// Absolute.
    Url(String),
}

impl Bookmark {
    /// Its target; `None` when it has both or neither of `path` and `url`.
    pub fn target(&self) -> Option<BookmarkTarget> {
        match (&self.path, &self.url) {
            (Some(path), None) => Some(BookmarkTarget::Path(path.clone())),
            (None, Some(url)) => Some(BookmarkTarget::Url(url.clone())),
            _ => None,
        }
    }

    /// Its URL with `base` (the environment's URL) for a path; `None` for a path without a base,
    /// a bookmark without one target, or a result that is no `http`/`https` URL (a base or `url`
    /// that never passed the check: nothing opens `file:` or `data:` from here). A path in a form
    /// the check refuses, or a result on another origin than `base`, is `None` too: a relative
    /// bookmark never leads to a host the environment did not name.
    pub fn url_with(&self, base: Option<&str>) -> Option<String> {
        let url = match self.target()? {
            BookmarkTarget::Url(url) => url,
            BookmarkTarget::Path(path) => {
                let base = base?;
                let url = crate::bookmark_paths::url_in(base, &path);
                let of_base = rb_util::url::origin_of(base);
                if !is_bookmark_path(&path)
                    || of_base.is_none()
                    || of_base != rb_util::url::origin_of(&url)
                {
                    return None;
                }
                url
            }
        };
        is_web_url(&url).then_some(url)
    }

    /// Whether it follows the environment: its target is a `path`.
    pub fn is_relative(&self) -> bool {
        matches!(self.target(), Some(BookmarkTarget::Path(_)))
    }
}

/// A bookmark with its URL in one environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBookmark {
    /// The bookmark's id.
    pub id: String,
    /// Its label.
    pub label: String,
    /// Its group.
    pub group: Option<String>,
    /// Its direct shortcut, as written.
    pub shortcut: Option<String>,
    /// Where it leads.
    pub url: String,
}

/// The URL a `path` is put after in `environment`: that environment's URL, else the origin of
/// `startUrl`, else the first environment's URL; `None` without any.
pub fn base_url(config: &ProjectConfig, environment: Option<&str>) -> Option<String> {
    environment
        .and_then(|id| config.environments.iter().find(|env| env.id == id))
        .map(|env| env.url.clone())
        .or_else(|| {
            config
                .start_url
                .as_deref()
                .and_then(origin)
                .map(str::to_owned)
        })
        .or_else(|| config.environments.first().map(|env| env.url.clone()))
}

/// The URLs of every bookmark in `environment` ([`base_url`]), in config order; a path without a
/// base and a URL that is no `http`/`https` URL are left out. The source of the sitemap capture (RB-050-05).
pub fn resolve(config: &ProjectConfig, environment: Option<&str>) -> Vec<String> {
    resolved(config, environment)
        .into_iter()
        .map(|bookmark| bookmark.url)
        .collect()
}

/// Every bookmark with its URL in `environment`, in config order (see [`resolve`]).
pub fn resolved(config: &ProjectConfig, environment: Option<&str>) -> Vec<ResolvedBookmark> {
    let base = base_url(config, environment);
    config
        .bookmarks
        .iter()
        .filter_map(|bookmark| {
            Some(ResolvedBookmark {
                url: bookmark.url_with(base.as_deref())?,
                id: bookmark.id.clone(),
                label: bookmark.label.clone(),
                group: bookmark.group.clone(),
                shortcut: bookmark.shortcut.clone(),
            })
        })
        .collect()
}

/// `current` as a target under `environment_url`: the part after it as a path when the page
/// lies under it (the same origin, the base's path on a segment boundary) and that part is a
/// path the check takes, else `current` as it is.
pub fn suggest_target(current: &str, environment_url: Option<&str>) -> BookmarkTarget {
    let rest = environment_url
        .and_then(|base| crate::bookmark_paths::rest_under(current, base))
        .map(crate::bookmark_paths::as_path)
        .filter(|path| is_bookmark_path(path));
    match rest {
        Some(path) => BookmarkTarget::Path(path),
        None => BookmarkTarget::Url(current.to_owned()),
    }
}

/// A new id from `label` (lowercase ASCII letters and digits joined by `-`; `bookmark` when none
/// is left) that none of `taken` has.
pub fn new_id(label: &str, taken: &[String]) -> String {
    let mut slug = String::new();
    for word in label
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
    {
        if !slug.is_empty() {
            slug.push('-');
        }
        slug.push_str(&word.to_ascii_lowercase());
    }
    if slug.is_empty() {
        slug.push_str("bookmark");
    }
    let mut id = slug.clone();
    let mut number = 2;
    while taken.contains(&id) {
        id = format!("{slug}-{number}");
        number += 1;
    }
    id
}

/// Every problem of `bookmarks[]`: an empty id or label, a repeated id, both or neither of
/// `path` and `url`, a path or URL in a form rBrowser does not open.
pub fn check_bookmarks(config: &ProjectConfig) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    let mut seen = HashSet::new();
    for (index, bookmark) in config.bookmarks.iter().enumerate() {
        let at = format!("/{BOOKMARKS_KEY}/{index}");
        issues.extend(problems(bookmark, &at));
        if !bookmark.id.is_empty() && !seen.insert(bookmark.id.as_str()) {
            issues.push(issue(
                format!("{at}/id"),
                "config.error.duplicate_id",
                format!("id `{}` is used more than once", bookmark.id),
            ));
        }
    }
    issues
}

/// The problems of one bookmark at the JSON pointer `at`.
fn problems(bookmark: &Bookmark, at: &str) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    let name = &bookmark.id;
    if bookmark.id.trim().is_empty() {
        issues.push(issue(
            format!("{at}/id"),
            ID_KEY,
            "a bookmark needs an id".to_owned(),
        ));
    }
    if bookmark.label.trim().is_empty() {
        issues.push(issue(
            format!("{at}/label"),
            LABEL_KEY,
            format!("bookmark `{name}` needs a label"),
        ));
    }
    match bookmark.target() {
        None => issues.push(issue(
            at.to_owned(),
            TARGET_KEY,
            format!(
                "bookmark `{name}` needs either `path` or `url`, not both and not neither: \
                 `path` follows the environment, `url` stays the same"
            ),
        )),
        Some(BookmarkTarget::Path(path)) if !is_bookmark_path(&path) => issues.push(issue(
            format!("{at}/path"),
            PATH_KEY,
            format!(
                "bookmark `{name}`: path `{path}` must start with one `/` (no scheme, no \
                 `//host`) and hold no backslash, space or control character"
            ),
        )),
        Some(BookmarkTarget::Url(url)) if !is_web_url(&url) => issues.push(issue(
            format!("{at}/url"),
            URL_KEY,
            format!("bookmark `{name}`: url must be an absolute http or https URL"),
        )),
        Some(_) => {}
    }
    issues
}

/// Removes from the merged config value every bookmark that is not an object of the right
/// shape, breaks a rule of [`check_bookmarks`] or repeats an id kept already; one warning
/// ([`IGNORED_KEY`]) each, so the rest of the config loads. Anything but a list is left to the
/// type check.
pub fn strip_invalid(merged: &mut Value) -> Vec<ConfigIssue> {
    let Some(items) = merged.get_mut(BOOKMARKS_KEY).and_then(Value::as_array_mut) else {
        return Vec::new();
    };
    let mut warnings = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut index = 0;
    items.retain(|item| {
        let at = format!("/{BOOKMARKS_KEY}/{index}");
        index += 1;
        let why = match serde_json::from_value::<Bookmark>(item.clone()) {
            Err(error) => Some(format!("it is no bookmark ({error})")),
            Ok(bookmark) => match problems(&bookmark, &at).first() {
                Some(problem) => Some(problem.message.clone()),
                None if !seen.insert(bookmark.id.clone()) => {
                    Some(format!("id `{}` is used more than once", bookmark.id))
                }
                None => None,
            },
        };
        match why {
            Some(why) => {
                warnings.push(issue(at, IGNORED_KEY, format!("bookmark ignored: {why}")));
                false
            }
            None => true,
        }
    });
    warnings
}

/// Whether `path` is one a bookmark may hold: it starts with one `/` - not `//host` or
/// `/\host`, which a browser reads as another host - and holds no backslash (a browser reads
/// it as `/`), whitespace or control character. The one validator of every way a path gets in:
/// the config check, the loader, `bookmarks.add` and what navigation resolves.
pub fn is_bookmark_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path
            .chars()
            .any(|c| c == '\\' || c.is_whitespace() || c.is_control())
}

/// Whether `url` is an absolute `http`/`https` URL with a host and no whitespace or control
/// character.
pub fn is_web_url(url: &str) -> bool {
    origin(url).is_some() && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// `scheme://host[:port]` of an `http`/`https` URL; `None` for anything else.
fn origin(url: &str) -> Option<&str> {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    let scheme = if lower.starts_with("https://") {
        8
    } else if lower.starts_with("http://") {
        7
    } else {
        return None;
    };
    let rest = url.get(scheme..)?;
    let host_len = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    if host_len == 0 {
        return None;
    }
    url.get(..scheme + host_len)
}

fn issue(path: String, key: &str, message: String) -> ConfigIssue {
    ConfigIssue {
        path,
        key: key.to_owned(),
        message,
    }
}

#[cfg(test)]
#[path = "bookmarks_tests.rs"]
mod tests;
