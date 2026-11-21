//! `bookmarks.make_relative` as an edit of the two config files (RB-080-48): every bookmark
//! whose absolute `url` has a path under the environments of the project
//! (`rb_config::bookmark_paths::stored_path`: it leads to the same page as before) gets that
//! path instead, so it follows the environment from then on; any other stays as it is. Nothing
//! is converted silently - only this command does it. Pure; the file work is `super::store`.

use serde_json::{Map, Value};

const BOOKMARKS: &str = rb_config::bookmarks::BOOKMARKS_KEY;

/// What [`convert`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Converted {
    /// The ids converted, each once, in file order.
    pub ids: Vec<String>,
    /// Whether `config.json` changed.
    pub project: bool,
    /// Whether `config.local.json` changed.
    pub local: bool,
}

/// Replaces `url` by `path` in the bookmarks of `project` (`config.json`) and `local`
/// (`config.local.json`) wherever `path_of` knows a path for the URL. A bookmark both files
/// give a target (the local file changes the team's by id) is converted in both or in neither:
/// a `path` in one beside a `url` in the other would be a bookmark with both, which the loader
/// drops.
pub fn convert(
    project: &mut Value,
    local: &mut Value,
    path_of: impl Fn(&str) -> Option<String>,
) -> Converted {
    let in_project = plan(project, local, &path_of);
    let in_local = plan(local, project, &path_of);
    let mut converted = Converted {
        ids: Vec::new(),
        project: !in_project.is_empty(),
        local: !in_local.is_empty(),
    };
    for (document, planned) in [(project, in_project), (local, in_local)] {
        for (index, id, path) in planned {
            let Some(item) = items_mut(document)
                .and_then(|items| items.get_mut(index))
                .and_then(Value::as_object_mut)
            else {
                continue;
            };
            replace_url(item, &path);
            if !converted.ids.contains(&id) {
                converted.ids.push(id);
            }
        }
    }
    converted
}

/// The ids of the bookmarks in `document` that hold a target in a form the check refuses (a
/// path that is none, a URL that is no web URL, both at once): the commands leave them as they
/// are - the loader drops them - and name them.
pub fn invalid_ids(document: &Value) -> Vec<String> {
    items(document)
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?;
            let path = present(item, "path");
            let url = present(item, "url");
            let bad_path = path.is_some_and(|path| {
                !path
                    .as_str()
                    .is_some_and(rb_config::bookmarks::is_bookmark_path)
            });
            let bad_url =
                url.is_some_and(|url| !url.as_str().is_some_and(rb_util::url::is_web_url));
            (bad_path || bad_url || (path.is_some() && url.is_some())).then(|| id.to_owned())
        })
        .collect()
}

/// What to convert in `document`: index, id and path of every bookmark with a `url` (and no
/// `path`) that `path_of` knows a path for, unless `other` gives the same id a target that
/// cannot be converted too.
fn plan(
    document: &Value,
    other: &Value,
    path_of: &impl Fn(&str) -> Option<String>,
) -> Vec<(usize, String, String)> {
    items(document)
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let (id, path) = convertible(item, path_of)?;
            let blocked = items(other).iter().any(|theirs| {
                theirs.get("id").and_then(Value::as_str) == Some(id)
                    && (present(theirs, "url").is_some() || present(theirs, "path").is_some())
                    && convertible(theirs, path_of).is_none()
            });
            (!blocked).then(|| (index, id.to_owned(), path))
        })
        .collect()
}

/// The value of `key` in `item`; a JSON `null` is no value, as the config's reader has it.
fn present<'a>(item: &'a Value, key: &str) -> Option<&'a Value> {
    item.get(key).filter(|value| !value.is_null())
}

/// The id of `item` and the path of its `url`, when it is a bookmark with an id, a `url`
/// `path_of` knows a path for and no `path`.
fn convertible<'a>(
    item: &'a Value,
    path_of: &impl Fn(&str) -> Option<String>,
) -> Option<(&'a str, String)> {
    let id = item.get("id")?.as_str()?;
    if present(item, "path").is_some() {
        return None;
    }
    let path = path_of(present(item, "url")?.as_str()?)?;
    Some((id, path))
}

/// `path` in the place of `url`, every other key where it was; a `path` that was `null` goes.
fn replace_url(item: &mut Map<String, Value>, path: &str) {
    for (key, value) in std::mem::take(item) {
        match key.as_str() {
            "url" => {
                item.insert("path".to_owned(), Value::String(path.to_owned()));
            }
            "path" => {}
            _ => {
                item.insert(key, value);
            }
        }
    }
}

fn items(document: &Value) -> &[Value] {
    document
        .get(BOOKMARKS)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn items_mut(document: &mut Value) -> Option<&mut Vec<Value>> {
    document.get_mut(BOOKMARKS).and_then(Value::as_array_mut)
}

#[cfg(test)]
#[path = "relative_tests.rs"]
mod tests;
