//! What MCP's `navigate` may load (batch-3 security review, finding 3): `http:` and `https:`,
//! and a `file:` URL of a file inside the open project. The same holds for the pages MCP hands
//! the check plugins (`checks_run`) and the breakpoint analyser (`breakpoints_detect`,
//! RB-094-10) and the page `compare_engines` renders. The address bar and the CLI keep the
//! wider list of `host::nav` (`file:` anywhere, `about:`, `data:`, `chrome:`, `view-source:`):
//! a person chose those; an agent would read local files or Chromium's internal pages with them.
//!
//! A `file:` URL is decoded into its path as the browser reads it (a backslash is a slash, and
//! `file://C:\site` names a drive, RB-090-30) and refused before the file system is touched when
//! it has a `..` step or does not lie below the project root as written (a `file://server/...`
//! share never makes Windows connect). Then the file itself must be no link (never followed) and
//! its canonical path must lie inside the canonical project root, so no folder link on the way
//! leads out. A link swapped in between this check and the load is the residual risk
//! (`docs/security/threat-model.md`).

use std::path::{Component, Path};

use rb_commands::CommandError;
use serde_json::Value;

/// The command MCP's `navigate` runs per target.
pub const NAVIGATE: &str = "view.navigate";

/// The command MCP's `checks_run` runs per target: its `url` is the page the check plugins read.
pub const CHECKS_RUN: &str = "checks.run_all";

/// The command MCP's `breakpoints_detect` runs: its `urls` are the pages the analyser loads in a
/// hidden view and whose style sheets it reads.
pub const ANALYZE: &str = "breakpoints.analyze";

/// Refuses the command `id` an MCP client runs when one of its URLs is one [`check`] refuses:
/// [`NAVIGATE`]'s `url` (required), [`CHECKS_RUN`]'s `url` and [`ANALYZE`]'s `urls` (both
/// optional; the views' own pages or the project's start URL without them, RB-094-10). Every
/// other command passes. Only the MCP backend calls this: the person's commands (palette,
/// shortcut, the analyser's wizard, the findings panel) keep the address bar's wider list.
///
/// # Errors
///
/// [`CommandError::InvalidArgs`] with the reason.
pub fn check_command(
    id: &str,
    args: &Value,
    project_root: Option<&Path>,
) -> Result<(), CommandError> {
    let invalid = CommandError::InvalidArgs;
    let urls: Vec<&str> = match id {
        NAVIGATE => vec![
            args.get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("`url` is missing".to_owned()))?,
        ],
        CHECKS_RUN => match args.get("url") {
            None | Some(Value::Null) => Vec::new(),
            Some(url) => vec![
                url.as_str()
                    .ok_or_else(|| invalid("`url` is no string".to_owned()))?,
            ],
        },
        ANALYZE => match args.get("urls") {
            None | Some(Value::Null) => Vec::new(),
            Some(urls) => urls
                .as_array()
                .ok_or_else(|| invalid("`urls` is no list".to_owned()))?
                .iter()
                .map(|url| {
                    url.as_str()
                        .ok_or_else(|| invalid("`urls` holds a value that is no string".to_owned()))
                })
                .collect::<Result<_, _>>()?,
        },
        _ => return Ok(()),
    };
    for url in urls {
        check(url, project_root).map_err(invalid)?;
    }
    Ok(())
}

/// `url` completed as the address bar completes it, when MCP may load it; else why not.
///
/// # Errors
///
/// A scheme other than `http`, `https` or `file`; a `file:` URL without an open project, with a
/// `..` step, outside the project, of a link, or of a file that cannot be read.
pub fn check(url: &str, project_root: Option<&Path>) -> Result<String, String> {
    let url = crate::host::nav::normalize_url(url).map_err(|error| error.to_string())?;
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .unwrap_or_default();
    match scheme.as_str() {
        "http" | "https" => Ok(url),
        "file" => check_file(&url, project_root).map(|()| url),
        _ => Err(format!(
            "MCP may load only http and https URLs, and file URLs inside the project; \
             `{scheme}:` is refused"
        )),
    }
}

fn check_file(url: &str, project_root: Option<&Path>) -> Result<(), String> {
    let root = project_root.ok_or("a file URL needs an open project; no project is open")?;
    let path = crate::analyzer::fetch::file_path(url).ok_or("the file URL does not decode")?;
    let outside = || "the file lies outside the project".to_owned();
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(outside());
    }
    if !path.starts_with(root) {
        return Err(outside());
    }
    let unreadable = |_| "the file cannot be read".to_owned();
    let meta = std::fs::symlink_metadata(&path).map_err(unreadable)?;
    if meta.file_type().is_symlink() {
        return Err("the file is a link; MCP never follows one".to_owned());
    }
    let real_root = std::fs::canonicalize(root).map_err(unreadable)?;
    let real_path = std::fs::canonicalize(&path).map_err(unreadable)?;
    if real_path.starts_with(&real_root) {
        Ok(())
    } else {
        Err(outside())
    }
}

#[cfg(test)]
#[path = "url_policy_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "url_inputs_tests.rs"]
mod url_inputs;
