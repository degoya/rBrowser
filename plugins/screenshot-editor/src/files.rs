//! Finding the screenshots: the newest first, by name. rBrowser names each capture run's folder
//! after its time (`.rbrowser/out/screenshots/2026-11-12_10-00-00/mobile.png`, RB-050-03), so the
//! greatest name at each level is the newest run; the walk goes down the greatest names first
//! and stops after [`MAX_LISTED`] images, [`MAX_DEPTH`] levels or [`MAX_LISTINGS`] folders.

use rb_plugin_api::{Failure, FailureKind, PluginHost};

use crate::paths::{OUT_ROOT, OWN_ROOT};

/// Where rBrowser's screenshots go unless `capture.dir` says otherwise.
pub const SCREENSHOTS: &str = ".rbrowser/out/screenshots";
/// The most images [`list`] answers.
pub const MAX_LISTED: usize = 200;
/// The deepest folder the walk opens below its start.
pub const MAX_DEPTH: usize = 6;
/// The most folders one walk lists.
pub const MAX_LISTINGS: usize = 128;

struct Walk {
    found: Vec<String>,
    max: usize,
    listings: usize,
    truncated: bool,
}

fn is_png(name: &str) -> bool {
    name.len() > 4
        && name
            .get(name.len() - 4..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(".png"))
}

/// Walks `dir` (no trailing `/`): files and folders, the greatest name first. A folder that does
/// not answer is skipped; only the start's failure other than "not found" is the caller's.
fn walk(
    host: &mut dyn PluginHost,
    dir: &str,
    depth: usize,
    skip: &[&str],
    walk: &mut Walk,
) -> Result<(), Failure> {
    if walk.listings >= MAX_LISTINGS || walk.found.len() >= walk.max {
        walk.truncated = true;
        return Ok(());
    }
    walk.listings += 1;
    let mut names = match host.project_list(dir) {
        Ok(names) => names,
        Err(failure) if depth > 0 || failure.kind == FailureKind::NotFound => return Ok(()),
        Err(failure) => return Err(failure),
    };
    names.sort_unstable_by(|a, b| b.cmp(a));
    for name in names {
        if walk.truncated {
            return Ok(());
        }
        // The host marks a folder with a trailing `/`.
        let path = format!("{dir}/{name}");
        if let Some(folder) = path.strip_suffix('/') {
            if depth < MAX_DEPTH && !skip.contains(&path.as_str()) {
                self::walk(host, folder, depth + 1, skip, walk)?;
            }
        } else if is_png(&name) {
            if walk.found.len() >= walk.max {
                walk.truncated = true;
                return Ok(());
            }
            walk.found.push(path);
        }
    }
    Ok(())
}

/// The newest screenshot below `.rbrowser/out/screenshots/`, if any.
pub fn last(host: &mut dyn PluginHost) -> Result<Option<String>, Failure> {
    let mut found = Walk {
        found: Vec::new(),
        max: 1,
        listings: 0,
        truncated: false,
    };
    walk(host, SCREENSHOTS, 0, &[], &mut found)?;
    Ok(found.found.into_iter().next())
}

/// The PNG files the editor can open: the screenshots newest first, then the rest of
/// `.rbrowser/out/` (never the editor's own folder); whether the list was cut.
pub fn list(host: &mut dyn PluginHost) -> Result<(Vec<String>, bool), Failure> {
    let mut found = Walk {
        found: Vec::new(),
        max: MAX_LISTED,
        listings: 0,
        truncated: false,
    };
    walk(host, SCREENSHOTS, 0, &[], &mut found)?;
    let screenshots = format!("{SCREENSHOTS}/");
    let out = OUT_ROOT.trim_end_matches('/');
    walk(host, out, 0, &[OWN_ROOT, &screenshots], &mut found)?;
    Ok((found.found, found.truncated))
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
