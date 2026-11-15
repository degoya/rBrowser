//! Map Local (RB-080-37): the file a request gets instead of the server's answer. A rule names a
//! folder (the URL's path below it) or one file; both are allowed only below the project or one
//! of the folders of `network-panel.mapLocalRoots`, and the resolved file must stay below its
//! folder after every `..` and symlink (canonical paths on both sides). Nothing else of the disk
//! is reachable through a rule.

use std::path::{Path, PathBuf};

/// Why no file was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalError {
    /// The path would leave the folder.
    Outside,
    /// No such file below the folder.
    NotFound,
}

/// The canonical path of `given` - absolute, or relative to the project - when it lies below
/// the project or one of `roots`.
///
/// # Errors
///
/// No project open for a relative path, the path does not exist, or it lies outside the project
/// and the allowed roots.
pub fn resolve(
    given: &str,
    project: Option<&Path>,
    roots: &[String],
) -> Result<PathBuf, &'static str> {
    let path = Path::new(given);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project.ok_or("no project open")?.join(path)
    };
    let canonical = joined.canonicalize().map_err(|_| "folder does not exist")?;
    let mut allowed: Vec<PathBuf> = roots
        .iter()
        .filter_map(|root| Path::new(root).canonicalize().ok())
        .collect();
    if let Some(project) = project.and_then(|project| project.canonicalize().ok()) {
        allowed.push(project);
    }
    if allowed.iter().any(|root| canonical.starts_with(root)) {
        Ok(canonical)
    } else {
        Err("folder outside the project and the allowed roots")
    }
}

/// The percent-decoded bytes of `text`.
fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(hex) = text.get(i + 1..i + 3)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// The file `url`'s path names below `folder` (canonical): each segment plain, `index.html`
/// for a folder; the result canonical and still below `folder`.
///
/// # Errors
///
/// [`LocalError::Outside`] for a path that leaves the folder or is no plain path,
/// [`LocalError::NotFound`] when there is no such file.
pub fn file_for(folder: &Path, url: &str) -> Result<PathBuf, LocalError> {
    let path = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    let path = path.find('/').map_or("", |at| &path[at..]);
    let decoded = percent_decode(path);
    let decoded = std::str::from_utf8(&decoded).map_err(|_| LocalError::Outside)?;
    let mut file = folder.to_path_buf();
    for segment in decoded.split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "."
            || segment == ".."
            || segment.contains(['\\', ':'])
            || segment.chars().any(char::is_control)
        {
            return Err(LocalError::Outside);
        }
        file.push(segment);
    }
    if file.is_dir() {
        file.push("index.html");
    }
    let canonical = file.canonicalize().map_err(|_| LocalError::NotFound)?;
    if !canonical.starts_with(folder) || !canonical.is_file() {
        return Err(LocalError::Outside);
    }
    Ok(canonical)
}

/// The MIME type of `path` by its extension; `application/octet-stream` for an unknown one.
#[must_use]
pub fn mime_of(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match extension.as_str() {
        "html" | "htm" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" | "map" => "application/json",
        "xml" => "application/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "txt" | "md" => "text/plain",
        "csv" => "text/csv",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
#[path = "local_tests.rs"]
mod tests;
