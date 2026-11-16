//! The flows of a project: `<root>/<automation.flows.dir>/<name>.ts` (default
//! `.rbrowser/flows`), their optional head comments (`// @description ...`, `// @views a,b`,
//! `// @timeout 30000`) and the source of one. Listing reads files only; running is the
//! orchestration's, after the trust check.
//!
//! The folder must stay inside `.rbrowser/flows`: workspace trust hashes that folder (K9), so a
//! flow anywhere else would run without anyone having trusted it.

use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use super::FlowError;

/// The protected folder every flows folder lies in.
pub const PROTECTED_DIR: &str = ".rbrowser/flows";
/// The extension of a flow file.
pub const EXTENSION: &str = "ts";
/// The longest flow file read.
pub const MAX_SOURCE_BYTES: u64 = 1024 * 1024;
/// The most flows listed.
pub const MAX_FLOWS: usize = 500;
/// The head comments are read from this many first lines.
const HEAD_LINES: usize = 30;

/// One flow as `flow_list` and the CLI show it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowInfo {
    /// The file name without `.ts`.
    pub name: String,
    /// Relative to the project root, with `/`.
    pub path: String,
    /// `// @description`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `// @views`: breakpoint ids or view labels the flow runs in when none are asked for.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<String>,
    /// `// @timeout`, in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u32>,
}

/// The head comments of a flow.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Head {
    /// `// @description`.
    pub description: Option<String>,
    /// `// @views`.
    pub views: Vec<String>,
    /// `// @timeout`.
    pub timeout_ms: Option<u32>,
}

/// Whether `name` is a flow name: letters, digits, `-`, `_` and `.` inside, at most 64.
pub fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !name.starts_with('.')
        && !name.ends_with('.')
}

/// The flows folder of `root` for the setting `dir`.
///
/// # Errors
///
/// [`FlowError::BadDir`] when `dir` is absolute, climbs with `..` or leaves `.rbrowser/flows`.
pub fn flows_dir(root: &Path, dir: &str) -> Result<PathBuf, FlowError> {
    let relative = dir.replace('\\', "/");
    let normal = Path::new(&relative)
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    let inside = relative
        .trim_start_matches("./")
        .strip_prefix(PROTECTED_DIR)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'));
    if !normal || !inside {
        return Err(FlowError::BadDir(dir.to_owned()));
    }
    Ok(root.join(relative.trim_start_matches("./")))
}

/// Reads the head comments of `source`: `// @key value` lines among the first lines.
pub fn parse_head(source: &str) -> Head {
    let mut head = Head::default();
    for line in source.lines().take(HEAD_LINES) {
        let Some(rest) = line.trim().strip_prefix("//") else {
            continue;
        };
        let rest = rest.trim();
        let (key, value) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let value = value.trim();
        match key {
            "@description" if !value.is_empty() => head.description = Some(value.to_owned()),
            "@views" => {
                head.views = value
                    .split(',')
                    .map(str::trim)
                    .filter(|view| !view.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            "@timeout" => head.timeout_ms = value.parse().ok().filter(|ms| *ms > 0),
            _ => {}
        }
    }
    head
}

/// Every flow in `dir` of `root`, by name; an absent folder has none.
///
/// # Errors
///
/// [`FlowError::BadDir`], or [`FlowError::Io`] when the folder cannot be read.
pub fn list(root: &Path, dir: &str) -> Result<Vec<FlowInfo>, FlowError> {
    let folder = flows_dir(root, dir)?;
    let entries = match std::fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(FlowError::Io(error.to_string())),
    };
    let mut flows: Vec<FlowInfo> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = flow_name(&path)?;
            let source = read_limited(&path).ok()?;
            let head = parse_head(&source);
            Some(FlowInfo {
                path: relative(root, &path),
                name,
                description: head.description,
                views: head.views,
                timeout_ms: head.timeout_ms,
            })
        })
        .take(MAX_FLOWS)
        .collect();
    flows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(flows)
}

/// The flow `name` of `root`: its file and source.
///
/// # Errors
///
/// [`FlowError::BadName`], [`FlowError::NotFound`] (with the known names), or
/// [`FlowError::Io`] for a file that cannot be read or is too long.
pub fn read(root: &Path, dir: &str, name: &str) -> Result<(PathBuf, String), FlowError> {
    if !valid_name(name) {
        return Err(FlowError::BadName(name.to_owned()));
    }
    let path = flows_dir(root, dir)?.join(format!("{name}.{EXTENSION}"));
    if !path.is_file() {
        let known = list(root, dir)
            .unwrap_or_default()
            .into_iter()
            .map(|flow| flow.name)
            .collect();
        return Err(FlowError::NotFound {
            name: name.to_owned(),
            known,
        });
    }
    let source = read_limited(&path)?;
    Ok((path, source))
}

/// `path` relative to `root`, with `/`.
pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn flow_name(path: &Path) -> Option<String> {
    if !path.is_file() || path.extension()? != EXTENSION {
        return None;
    }
    let name = path.file_stem()?.to_str()?;
    valid_name(name).then(|| name.to_owned())
}

fn read_limited(path: &Path) -> Result<String, FlowError> {
    let length = std::fs::metadata(path)
        .map_err(|error| FlowError::Io(error.to_string()))?
        .len();
    if length > MAX_SOURCE_BYTES {
        return Err(FlowError::Io(format!(
            "{} is longer than {MAX_SOURCE_BYTES} bytes",
            path.display()
        )));
    }
    std::fs::read_to_string(path).map_err(|error| FlowError::Io(error.to_string()))
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
