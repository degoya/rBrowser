//! The Firefox line of the About page and of MCP's `app_info` (RB-090-49): the Firefox rBrowser
//! starts - the setting `engines.firefox.path` when it is set and present, else the first
//! installation found, in the order the start takes it (`rb_bidi::launcher::find_firefox`) -
//! with its version from `application.ini`. The installations come from the cache of
//! `engines::firefox::installed`, so answering never walks the registry or the disk.

use std::path::Path;

use rb_bidi::launcher::Installed;
use rb_ipc::AboutInfo;

use super::facts::UNKNOWN;

/// `info` with the Firefox of `installed` and the setting `configured` (blank is unset);
/// `exists` tells whether a set path is there. Nothing found, or a set path that is missing:
/// unchanged (the page says "not found" and offers the engines' settings).
pub fn with_firefox(
    mut info: AboutInfo,
    installed: &[Installed],
    configured: Option<&str>,
    exists: impl Fn(&Path) -> bool,
) -> AboutInfo {
    let configured = configured.map(str::trim).filter(|path| !path.is_empty());
    let chosen = match configured {
        Some(path) => exists(Path::new(path)).then(|| {
            let version = installed
                .iter()
                .find(|found| found.path == Path::new(path))
                .and_then(|found| found.version.clone());
            (path.to_owned(), version)
        }),
        None => installed
            .first()
            .map(|found| (found.path.display().to_string(), found.version.clone())),
    };
    if let Some((path, version)) = chosen {
        info.firefox = Some(version.unwrap_or_else(|| UNKNOWN.to_owned()));
        info.firefox_path = Some(path);
    }
    info
}

#[cfg(test)]
#[path = "firefox_tests.rs"]
mod tests;
