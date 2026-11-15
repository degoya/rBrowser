//! The stylesheets of a page without a browser, for `rbrowser init` before any window exists:
//! the HTML is fetched, every `<link rel="stylesheet">` and `<style>` it names is read, and
//! every `@import` followed (three levels). Cross-origin sheets come along like same-origin
//! ones. What a script adds at run time is missing here; the wizard in the app reads the page in
//! a hidden view over CDP instead ([`super::cdp_source`]).
//!
//! `http`/`https` URLs are fetched with a timeout and a size limit; a `file:` page (a static
//! site on disk) may name `file:` sheets, an `http(s)` page never does.

use std::collections::BTreeSet;
use std::io::Read as _;
use std::path::Path;
use std::time::Duration;

use super::html::{SheetRef, imports, stylesheets};
use super::source::{Collected, MAX_SHEET_BYTES, MAX_SHEETS, Sheet};

/// How long one request may take.
const TIMEOUT: Duration = Duration::from_secs(15);
/// The largest HTML document read.
const MAX_HTML_BYTES: u64 = 4 * 1024 * 1024;
/// How deep `@import`s are followed.
const MAX_IMPORT_DEPTH: usize = 3;

/// Reads a URL's body as text.
pub trait Fetch {
    /// The body of `url`, at most `limit` bytes.
    ///
    /// # Errors
    ///
    /// The request failed, answered no success or was too large; the message is English.
    fn text(&self, url: &str, limit: u64) -> Result<String, String>;
}

/// The network and the disk.
pub struct HttpFetch {
    agent: ureq::Agent,
}

impl Default for HttpFetch {
    fn default() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .build()
            .into();
        Self { agent }
    }
}

impl Fetch for HttpFetch {
    fn text(&self, url: &str, limit: u64) -> Result<String, String> {
        if let Some(path) = file_path(url) {
            return read_file(&path, limit).map_err(|error| format!("{url}: {error}"));
        }
        let mut response = self
            .agent
            .get(url)
            .call()
            .map_err(|error| format!("{url}: {error}"))?;
        response
            .body_mut()
            .with_config()
            .limit(limit)
            .read_to_string()
            .map_err(|error| format!("{url}: {error}"))
    }
}

/// At most `limit` bytes of the regular file at `path`: never more is read, and a folder, a
/// pipe or a device (which could block or never end) is refused.
fn read_file(path: &Path, limit: u64) -> Result<String, String> {
    let not_regular = || "not a regular file".to_owned();
    if !std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err(not_regular());
    }
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    // The handle again: the path may have changed since.
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err(not_regular());
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("larger than {limit} bytes"));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The local path of a `file:` URL (`file:///C:/site/a.css`, `file:///home/me/a.css`,
/// `file://server/share/a.css` as the UNC path `//server/share/a.css`), read as
/// [`normalize_file_url`] reads it, its percent-escapes decoded; `None` for another scheme, a
/// broken escape or a path that is no UTF-8.
pub(crate) fn file_path(url: &str) -> Option<std::path::PathBuf> {
    let url = normalize_file_url(url);
    if !is_file(&url) || !url.get(5..)?.starts_with("//") {
        return None;
    }
    let rest = url.get(7..)?;
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let path = percent_decode(path)?;
    let path = if host.is_empty() || host.eq_ignore_ascii_case("localhost") {
        path
    } else {
        format!("//{}{path}", percent_decode(host)?)
    };
    // `/C:/site` on Windows is `C:/site`.
    let path = match path.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => path[1..].to_owned(),
        _ => path,
    };
    Some(std::path::PathBuf::from(path))
}

/// A `file:` URL as a browser reads it (WHATWG URL, RB-090-30): a backslash before the query is
/// a slash, and a Windows drive in the host's place is the path's first segment, so
/// `file://C:\site\a.css` is `file:///C:/site/a.css`, not a share on a server `C:`. `..` steps
/// and escapes stay; another URL, or a `file:` URL without `//`, comes back as it is.
pub(crate) fn normalize_file_url(url: &str) -> String {
    let Some(rest) = url.get(5..).filter(|_| is_file(url)) else {
        return url.to_owned();
    };
    let (before, suffix) = rest.split_at(rest.find(['?', '#']).unwrap_or(rest.len()));
    let before = before.replace('\\', "/");
    let Some(after) = before.strip_prefix("//") else {
        return url.to_owned();
    };
    let host = after.split('/').next().unwrap_or_default();
    let drive = matches!(host.as_bytes(), [letter, b':'] if letter.is_ascii_alphabetic());
    let slash = if drive { "/" } else { "" };
    format!("file://{slash}{after}{suffix}")
}

/// `text` with every `%XX` decoded; `None` for a broken escape or bytes that are no UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'%' {
            let digits = bytes.get(index + 1..index + 3)?;
            if !digits.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            let digits = std::str::from_utf8(digits).ok()?;
            decoded.push(u8::from_str_radix(digits, 16).ok()?);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn is_file(url: &str) -> bool {
    url.get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("file:"))
}

/// The stylesheets of every page of `urls`, each sheet once.
///
/// # Errors
///
/// No page could be read at all; a sheet that fails is left out with a warning in the log.
pub fn collect(fetch: &dyn Fetch, urls: &[String]) -> Result<Collected, String> {
    let mut collected = Collected::default();
    let mut seen = BTreeSet::new();
    let mut failures = Vec::new();
    for page in urls {
        // The base the sheets resolve against, with slashes where a person typed backslashes.
        let page = &normalize_file_url(page);
        let html = match fetch.text(page, MAX_HTML_BYTES) {
            Ok(html) => html,
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        let mut inline = 0;
        for found in stylesheets(&html, page) {
            match found {
                SheetRef::Inline { text, media } => {
                    inline += 1;
                    let url = format!("{page}#inline-{inline}");
                    read_imports(fetch, page, &text, 1, &mut seen, &mut collected);
                    collected.push(Sheet { url, text, media });
                }
                SheetRef::Link { url, media } => {
                    if is_file(&url) && !is_file(page) {
                        continue;
                    }
                    if seen.insert(url.clone()) {
                        read_sheet(fetch, &url, media, 0, &mut seen, &mut collected);
                    }
                }
            }
        }
    }
    if collected.sheets.is_empty() && !failures.is_empty() {
        return Err(failures.join("; "));
    }
    Ok(collected)
}

fn read_sheet(
    fetch: &dyn Fetch,
    url: &str,
    media: Option<String>,
    depth: usize,
    seen: &mut BTreeSet<String>,
    collected: &mut Collected,
) {
    if collected.sheets.len() >= MAX_SHEETS {
        return;
    }
    match fetch.text(url, MAX_SHEET_BYTES as u64) {
        Ok(text) => {
            read_imports(fetch, url, &text, depth + 1, seen, collected);
            collected.push(Sheet {
                url: url.to_owned(),
                text,
                media,
            });
        }
        Err(error) => tracing::warn!(%error, "a stylesheet was not read"),
    }
}

fn read_imports(
    fetch: &dyn Fetch,
    base: &str,
    text: &str,
    depth: usize,
    seen: &mut BTreeSet<String>,
    collected: &mut Collected,
) {
    if depth > MAX_IMPORT_DEPTH {
        return;
    }
    for url in imports(text, base) {
        if is_file(&url) && !is_file(base) {
            continue;
        }
        if seen.insert(url.clone()) {
            read_sheet(fetch, &url, None, depth, seen, collected);
        }
    }
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
