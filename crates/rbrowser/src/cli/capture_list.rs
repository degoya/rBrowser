//! `rbrowser capture sitemap --list` and `--urls <file>` (RB-099-09): the discovery without a
//! capture, printed one URL a line (`--json`: the whole answer of `sitemap_discover`), and a
//! capture of exactly the URLs of a file - the running rBrowser discovers the source again and
//! refuses a URL it does not find.
//!
//! Neither list goes over the instance channel, whose request line and reply are short
//! (`protocol::MAX_LINE`, `protocol::MAX_REPLY`): `--urls` names a `.txt` file inside the project,
//! which the running rBrowser reads (at most `MAX_PAGES` URLs); `--list` reads the discovery from
//! the file the running rBrowser wrote below `.rbrowser/out/sitemap-discover/`.

use std::path::{Component, Path};

use rb_ipc::SitemapDiscovery;

use super::capture::{SitemapCliArgs, answered};
use super::exit;
use crate::instance::client::{self, Outcome};
use crate::instance::protocol::Request;
use crate::sitemap::discover_tool::{DiscoverArgs, DiscoveryFile};
use crate::sitemap::discovery::{DISCOVER_DIR, MAX_URLS_FILE_BYTES};
use crate::sitemap::tool::CaptureArgs;

/// `file` (relative to `cwd`) as the project-relative path the running rBrowser reads, `/`
/// between its parts.
///
/// # Errors
///
/// The file is not a `.txt` file inside the project `root`, or larger than allowed.
pub fn project_relative(file: &Path, cwd: &Path, root: &Path) -> Result<String, String> {
    let path = super::paths::absolute(file, cwd);
    let named = path.display();
    let inside = path
        .strip_prefix(root)
        .map_err(|_| format!("{named} is not inside the project {}", root.display()))?;
    let parts: Vec<String> = inside
        .components()
        .map(|part| match part {
            Component::Normal(name) => Ok(name.to_string_lossy().into_owned()),
            _ => Err(format!("{named} is not a plain path inside the project")),
        })
        .collect::<Result<_, _>>()?;
    let relative = parts.join("/");
    if !relative.to_ascii_lowercase().ends_with(".txt") {
        return Err(format!("{named} is no .txt file"));
    }
    let size = std::fs::metadata(&path)
        .map_err(|error| format!("{named}: {error}"))?
        .len();
    if size > MAX_URLS_FILE_BYTES {
        return Err(format!(
            "{named} is larger than {MAX_URLS_FILE_BYTES} bytes"
        ));
    }
    Ok(relative)
}

/// `capture` with the URL list `file` (relative to `cwd`, inside the project `root`), checked
/// again; unchanged without one.
///
/// # Errors
///
/// As [`project_relative`], or the arguments out of bounds.
pub fn with_urls_file(
    mut capture: CaptureArgs,
    file: Option<&Path>,
    cwd: &Path,
    root: &Path,
) -> Result<CaptureArgs, String> {
    let Some(file) = file else {
        return Ok(capture);
    };
    capture.urls_file = Some(project_relative(file, cwd, root)?);
    capture.check()?;
    Ok(capture)
}

/// The discovery the running rBrowser wrote for `written` below the project `root`
/// (`.rbrowser/out/sitemap-discover/<discoveryId>.json`; the path is made here from the plain
/// id, never taken from the answer).
///
/// # Errors
///
/// An id that is no plain name, or a file that cannot be read or parsed.
pub fn read_discovery(written: &DiscoveryFile, root: &Path) -> Result<SitemapDiscovery, String> {
    let id = &written.discovery_id;
    if !rb_visual::slug::valid_name(id) {
        return Err(format!("{id} is no plain discovery id"));
    }
    let mut file = root.to_path_buf();
    file.extend(DISCOVER_DIR);
    file.push(format!("{id}.json"));
    let named = file.display();
    let bytes = std::fs::read(&file).map_err(|error| format!("{named}: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{named}: {error}"))
}

/// The discovery's arguments of the command line.
#[must_use]
pub fn discover_args(args: &SitemapCliArgs) -> DiscoverArgs {
    let list = |items: &[String]| (!items.is_empty()).then(|| items.to_vec());
    DiscoverArgs {
        url: args.url.clone(),
        bookmarks: args.bookmarks,
        max: args.max,
        include: list(&args.include),
        exclude: list(&args.exclude),
        depth: None,
    }
}

/// What `--list` prints: every URL a line, or with `json` the whole discovery.
#[must_use]
pub fn listing(discovery: &SitemapDiscovery, json: bool) -> String {
    if json {
        return serde_json::to_string_pretty(discovery).unwrap_or_default();
    }
    discovery.pages.join("\n")
}

/// `rbrowser capture sitemap --list`: the hand-over and the listing; the exit code.
pub fn list(args: &SitemapCliArgs, root: &Path, local: Option<&Path>) -> i32 {
    let fail = |code: i32, message: &str| {
        tracing::error!(code, "capture sitemap --list: {message}");
        eprintln!("rbrowser capture sitemap: {message}");
        code
    };
    let discover = discover_args(args);
    if let Err(message) = discover.check() {
        return fail(exit::USAGE, &message);
    }
    let outcome = local.map_or(Outcome::NotRunning, |local| {
        client::hand_over(local, root, Request::SitemapDiscover { args: discover })
    });
    let read = answered::<DiscoveryFile>(outcome, root, "discovery")
        .and_then(|written| read_discovery(&written, root).map_err(|error| (exit::FAILED, error)));
    match read {
        Ok(discovery) => {
            println!("{}", listing(&discovery, args.json));
            if discovery.truncated {
                eprintln!("rbrowser capture sitemap: a limit cut the discovery short");
            }
            exit::OK
        }
        Err((code, message)) => fail(code, &message),
    }
}

#[cfg(test)]
#[path = "capture_list_tests.rs"]
mod tests;
