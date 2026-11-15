//! The Web Store's version of an extension without a download (RB-080-23, the update check):
//! the update server answers `response=updatecheck` with a small XML naming the version
//! (`<app appid="<id>"><updatecheck version="..."/></app>`, the Omaha protocol). It is read
//! with a bounded scan for the two elements and the one attribute - no XML dependency - over
//! the same hosts and hop checks as the download ([`super::webstore::fetch`]), at most
//! [`MAX_UPDATE_CHECK_BYTES`]. What it does not name is `None`; the caller downloads then.
//! Blocking: the callers run it on a blocking thread.

use super::manifest::is_valid_version;
use super::webstore::{DownloadError, HopFetcher, UPDATE_HOST, fetch, is_id};

/// The largest answer read; a real one is under 2 KiB.
pub const MAX_UPDATE_CHECK_BYTES: u64 = 64 * 1024;

/// The update server's address for the version of `id`, answering with XML.
pub fn update_check_url(id: &str, chromium_version: &str) -> String {
    format!(
        "https://{UPDATE_HOST}/service/update2/crx?response=updatecheck&prodversion={chromium_version}&acceptformat=crx3&x=id%3D{id}%26uc"
    )
}

/// Asks the update server for the store's version of `id`; `None` when the answer names none.
///
/// # Errors
///
/// A bad id, a hop off Google's hosts, an answer past the limit, the network.
pub fn check_version(
    fetcher: &dyn HopFetcher,
    id: &str,
    chromium_version: &str,
) -> Result<Option<String>, DownloadError> {
    if !is_id(id) {
        return Err(DownloadError::BadId);
    }
    let bytes = fetch(
        fetcher,
        update_check_url(id, chromium_version),
        MAX_UPDATE_CHECK_BYTES,
    )?;
    Ok(latest_version(&String::from_utf8_lossy(&bytes), id))
}

/// The `version` of the `updatecheck` element inside the `app` element of `id` in `xml`;
/// `None` when there is none, or it is no version.
pub fn latest_version(xml: &str, id: &str) -> Option<String> {
    let mut rest = xml;
    while let Some(start) = rest.find("<app") {
        let tag_start = &rest[start..];
        let after = tag_start.get(4..)?;
        if !after.starts_with(|c: char| c.is_ascii_whitespace()) {
            rest = after;
            continue;
        }
        let tag_end = tag_start.find('>')?;
        let tag = &tag_start[..tag_end];
        let body = &tag_start[tag_end + 1..];
        if attribute(tag, "appid").is_none_or(|found| found != id) {
            rest = body;
            continue;
        }
        if tag.ends_with('/') {
            return None;
        }
        let body = &body[..body.find("</app>").unwrap_or(body.len())];
        let check = body.find("<updatecheck")?;
        let check = &body[check..];
        let check = &check[..check.find('>')?];
        return attribute(check, "version")
            .filter(|version| is_valid_version(version))
            .map(str::to_owned);
    }
    None
}

/// The double-quoted value of the attribute `name` in `tag`, whitespace before the name.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    let wanted = format!("{name}=\"");
    while let Some(at) = rest.find(&wanted) {
        let before = rest[..at].chars().next_back();
        let value = &rest[at + wanted.len()..];
        if before.is_some_and(|c| c.is_ascii_whitespace()) {
            return value.split('"').next();
        }
        rest = value;
    }
    None
}

#[cfg(test)]
#[path = "update_check_tests.rs"]
mod tests;
