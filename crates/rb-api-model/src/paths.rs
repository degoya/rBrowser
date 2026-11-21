//! A request's id is its file's path below `.rbrowser/api/`, `/`-joined: `users/create.http`.
//! Folders are the collection's folders. An id is checked before any file is touched: relative,
//! no `.`/`..` segment or hidden name, a short list of characters, a few levels, `.http` at the
//! end - so no id leads out of the folder or onto a file another tool keeps there.

/// The folder of the collections, below the project root.
pub const API_DIR: &str = ".rbrowser/api";
/// The extension of a request's file.
pub const EXTENSION: &str = ".http";
/// The most folder levels above a request's file.
pub const MAX_DEPTH: usize = 4;
/// The longest id, in bytes.
pub const MAX_ID_BYTES: usize = 200;
/// The longest file name made from a title, without the extension.
pub const MAX_STEM_CHARS: usize = 60;

fn segment_ok(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.starts_with('.')
        && !segment.ends_with(' ')
        && segment.len() <= 80
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " _-.".contains(c))
}

/// Whether `id` is a request's id.
#[must_use]
pub fn is_request_id(id: &str) -> bool {
    id.len() <= MAX_ID_BYTES
        && id.ends_with(EXTENSION)
        && id.len() > EXTENSION.len()
        && is_folder(id)
}

/// Whether `folder` is a folder's path below the collections (`""` is the top).
#[must_use]
pub fn is_folder(folder: &str) -> bool {
    if folder.is_empty() {
        return true;
    }
    let segments: Vec<&str> = folder.split('/').collect();
    segments.len() <= MAX_DEPTH + 1 && segments.iter().all(|segment| segment_ok(segment))
}

/// The file name a request titled `name` gets: lower case, words joined by `-`, other characters
/// dropped, at most [`MAX_STEM_CHARS`]; `request` when nothing is left.
#[must_use]
pub fn file_name(name: &str) -> String {
    let mut stem = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            stem.push(c.to_ascii_lowercase());
        } else if (c.is_whitespace() || "-_./".contains(c)) && !stem.ends_with('-') {
            stem.push('-');
        }
    }
    let stem: String = stem
        .trim_matches('-')
        .chars()
        .take(MAX_STEM_CHARS)
        .collect();
    let stem = stem.trim_end_matches('-');
    format!(
        "{}{EXTENSION}",
        if stem.is_empty() { "request" } else { stem }
    )
}

/// The id of `file_name` in `folder`.
#[must_use]
pub fn join(folder: &str, file_name: &str) -> String {
    let folder = folder.trim_matches('/');
    if folder.is_empty() {
        file_name.to_owned()
    } else {
        format!("{folder}/{file_name}")
    }
}

/// `id` with `-2`, `-3`, ... before its extension until `taken` says no.
#[must_use]
pub fn free_id(id: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(id) {
        return id.to_owned();
    }
    let stem = id.strip_suffix(EXTENSION).unwrap_or(id);
    (2..1000)
        .map(|n| format!("{stem}-{n}{EXTENSION}"))
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| format!("{stem}-1000{EXTENSION}"))
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
