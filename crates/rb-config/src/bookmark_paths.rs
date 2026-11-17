//! The path of a URL under an environment (RB-080-48), once for everything that asks: the
//! bookmarks and the environment comparison (`rbrowser::compare::plan`). An environment's URL is
//! a base - an origin and maybe a path; a URL lies under it when the origins are the same
//! (compared as origins: scheme and host without case, the default port dropped) and the base's
//! path ends on a segment boundary of the URL's.
//!
//! [`path_in`] is what "bookmark this page" stores, [`stored_path`] what
//! `bookmarks.make_relative` turns an existing URL into. Both answer a path only when putting
//! it after the base again gives the same page, so no bookmark changes where it leads.

use crate::ProjectConfig;
use crate::bookmarks::{base_url, is_bookmark_path, is_web_url};

/// The origin (`scheme://host[:port]`, lower case, no default port, no user info) and the rest
/// of an `http`/`https` URL; `None` for anything else.
pub fn split(url: &str) -> Option<(String, &str)> {
    let origin = rb_util::url::origin_of(url)?;
    let authority = url.find("://")? + "://".len();
    let end = url
        .get(authority..)?
        .find(['/', '?', '#'])
        .map_or(url.len(), |offset| authority + offset);
    Some((origin, url.get(end..)?))
}

/// The part of `url` after the base `base` when `url` lies under it: empty, or starting with
/// `/`, `?` or `#`.
pub fn rest_under<'a>(url: &'a str, base: &str) -> Option<&'a str> {
    let (origin, rest) = split(url)?;
    let (base_origin, base_path) = split(base.trim_end_matches('/'))?;
    if origin != base_origin {
        return None;
    }
    let after = rest.strip_prefix(base_path)?;
    (after.is_empty() || after.starts_with(['/', '?', '#'])).then_some(after)
}

/// `rest` as a path: empty is `/`, a bare query or hash goes after `/`.
pub fn as_path(rest: &str) -> String {
    if rest.starts_with('/') {
        rest.to_owned()
    } else {
        format!("/{rest}")
    }
}

/// The URL of `path` under the base `base`.
pub fn url_in(base: &str, path: &str) -> String {
    format!("{}{path}", base.trim_end_matches('/'))
}

/// The path of `url` under `base`: one the bookmark check takes and that, put after `base`
/// again, is the same page. A URL with user info has none (the path would drop it).
fn path_under(url: &str, base: &str) -> Option<String> {
    if rb_util::url::authority_of(url) != authority_with_user(url) {
        return None;
    }
    let path = as_path(rest_under(url, base)?);
    let again = url_in(base, &path);
    let same = matches!(
        (split(&again), split(url)),
        (Some((a, rest_a)), Some((b, rest_b))) if a == b && as_path(rest_a) == as_path(rest_b)
    );
    (same && is_bookmark_path(&path)).then_some(path)
}

/// The authority of `url` as written, user info included.
fn authority_with_user(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    rest.split(['/', '?', '#']).next()
}

/// The path part of a base, without its trailing slashes.
fn base_path(base: &str) -> Option<&str> {
    split(base.trim_end_matches('/')).map(|(_, path)| path)
}

/// What "bookmark this page" stores for the page `url` while `environment` is in effect: its
/// path under the base in effect ([`base_url`]: that environment, else the start URL's origin,
/// else the first environment) when the page lies under it - the path is put after that base
/// again, so it decides. A page of another environment has a path only when that environment's
/// base has the same path part, so the bookmark opens the corresponding page here. `None` for
/// anything else: the bookmark keeps its URL.
pub fn path_in(config: &ProjectConfig, environment: Option<&str>, url: &str) -> Option<String> {
    if !is_web_url(url) {
        return None;
    }
    let in_effect = base_url(config, environment)?;
    if let Some(path) = path_under(url, &in_effect) {
        return Some(path);
    }
    let part = base_path(&in_effect)?;
    config
        .environments
        .iter()
        .filter(|other| base_path(&other.url) == Some(part))
        .find_map(|other| path_under(url, &other.url))
}

/// The path an existing bookmark `url` is turned into: the one it has under every base of the
/// project that names its host (the environments and the start URL's origin). `None` when no
/// base names the host, when one that does would not hold the page, or when two disagree on
/// the path - the path would open another page there than the URL did.
pub fn stored_path(config: &ProjectConfig, url: &str) -> Option<String> {
    if !is_web_url(url) {
        return None;
    }
    let (origin, _) = split(url)?;
    let start = config
        .start_url
        .as_deref()
        .and_then(|start| split(start).map(|(origin, _)| origin));
    let mut found: Option<String> = None;
    let bases = config
        .environments
        .iter()
        .map(|environment| environment.url.clone())
        .chain(start);
    for base in bases {
        if split(&base).is_none_or(|(of_base, _)| of_base != origin) {
            continue;
        }
        let path = path_under(url, &base)?;
        if found.as_ref().is_some_and(|known| *known != path) {
            return None;
        }
        found = Some(path);
    }
    found
}

#[cfg(test)]
#[path = "bookmark_paths_tests.rs"]
mod tests;
