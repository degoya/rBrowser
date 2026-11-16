//! The MCP coverage (RB-030-08): every command's MCP entry resolves, and every tool is
//! described, typed and named by a command or says why not.
//!
//! - A command is `covered(<tool>)`, the tool registered by this server, declared by a bundled
//!   plugin, or in [`PENDING`] with the one job that brings it; or `omitted(<reason>)`, the reason
//!   plain English ([`problems`]).
//! - Every registered tool is named by a command or listed in [`tables::WITHOUT_COMMAND`]
//!   ([`orphans`]).
//! - A pending job has a job file and is not done yet ([`pending_job_problems`]).
//! - The tools themselves: one name per tool, a description, an object schema, `target` where
//!   they act on views ([`tools::tool_problems`]); plugin tools `<mcp_prefix>_<tool>`
//!   ([`plugins::plugin_problems`]).
//!
//! The host's test runs [`check`] over every command it registers
//! (`crates/rbrowser/src/mcp/mod_tests.rs`); `tests/coverage.rs` runs every rule over the
//! checked-in `schema/commands.v1.json` and the bundled plugins' manifests. [`reference`] writes
//! `schema/mcp-tools.v1.json`, from which `scripts/mcp-reference.sh` renders the wiki's tool
//! reference.

pub mod plugins;
pub mod reference;
pub mod tables;
pub mod tools;

use std::path::Path;

use rb_commands::McpCoverage;

use crate::RbMcpServer;

/// The tools a command names that are registered later: tool and the job that brings it.
pub const PENDING: &[(&str, &str)] = &[];

/// The reason the plugin host gives a plugin command that declares neither `tool` nor
/// `mcp_omitted` (`rb-plugin-host`, `commands.rs`).
pub const UNDECLARED: &str = "undeclared";
/// The reason the plugin host gives a plugin command whose `tool` has no `mcp_prefix` to live
/// under.
pub const NO_PREFIX: &str = "no mcp_prefix";

/// The job that brings `tool` when it is pending.
#[must_use]
pub fn pending(tool: &str) -> Option<&'static str> {
    PENDING
        .iter()
        .find(|(name, _)| *name == tool)
        .map(|(_, job)| *job)
}

/// Every command (`entries`: id and MCP entry) whose `covered(<tool>)` resolves to no tool:
/// neither known (`tools`: the registered ones as [`RbMcpServer::tool_names`] gives them, and the
/// bundled plugins') nor pending; an `omitted` whose reason is not plain English; a pending tool
/// that is registered. Each as `<what>: <why>`.
#[must_use]
pub fn problems(entries: &[(&str, &McpCoverage)], tools: &[String]) -> Vec<String> {
    let mut problems: Vec<String> = entries
        .iter()
        .filter_map(|(id, mcp)| match mcp {
            McpCoverage::Covered(tool)
                if !tools.iter().any(|known| known == tool) && pending(tool).is_none() =>
            {
                Some(format!(
                    "{id}: covered({tool}) names no registered or pending tool"
                ))
            }
            McpCoverage::Covered(_) => None,
            McpCoverage::Omitted(reason) => {
                reason_problem(reason).map(|why| format!("{id}: omitted {why}"))
            }
        })
        .collect();
    problems.extend(
        PENDING
            .iter()
            .filter(|(tool, _)| tools.iter().any(|known| known == tool))
            .map(|(tool, _)| format!("{tool}: registered and still listed as pending")),
    );
    problems
}

/// What is wrong with an `omitted` reason, if anything: it must be plain English a reader of
/// the reference understands - not empty, ASCII, no i18n key, no job placeholder
/// (`mcp: RB-030-01`; a tool that is coming is `covered(<tool>)` plus a [`PENDING`] entry), not
/// the plugin host's stand-in for a missing declaration.
fn reason_problem(reason: &str) -> Option<String> {
    let text = reason.trim();
    if text.is_empty() {
        return Some("without a reason".to_owned());
    }
    if text == UNDECLARED {
        return Some("because it declares neither a tool nor a reason".to_owned());
    }
    if text == NO_PREFIX {
        return Some("because it names a tool but its plugin has no mcp_prefix".to_owned());
    }
    if !text.is_ascii() {
        return Some(format!("with a reason that is not ASCII English: {text}"));
    }
    if is_i18n_key(text) {
        return Some(format!(
            "with an i18n key as reason ({text}); the reason is plain English"
        ));
    }
    if let Some(job) = job_id_in(text) {
        return Some(format!(
            "with a reason that names the job {job}: a coming tool is covered(<tool>) with a \
             PENDING entry"
        ));
    }
    None
}

/// `commands.view.zoom_in`: lower-case words of letters, digits and `_`, joined by dots, no
/// space.
fn is_i18n_key(text: &str) -> bool {
    let words: Vec<&str> = text.split('.').collect();
    words.len() > 1
        && words.iter().all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        })
}

/// The first `RB-<3 digits>-<2 digits>` in `text`.
fn job_id_in(text: &str) -> Option<&str> {
    text.match_indices("RB-").find_map(|(at, _)| {
        let id = text.get(at..at + 9)?;
        let bytes = id.as_bytes();
        let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
        (digits(3..6) && bytes[6] == b'-' && digits(7..9)).then_some(id)
    })
}

/// Every registered tool (`tools`) that no command names and [`tables::WITHOUT_COMMAND`] does
/// not explain, and every entry of that table a command names after all.
#[must_use]
pub fn orphans(entries: &[(&str, &McpCoverage)], tools: &[String]) -> Vec<String> {
    let named = |tool: &str| {
        entries
            .iter()
            .any(|(_, mcp)| matches!(mcp, McpCoverage::Covered(name) if name == tool))
    };
    tools
        .iter()
        .filter_map(|tool| {
            let listed = tables::without_command(tool).is_some();
            match (named(tool), listed) {
                (false, false) => Some(format!(
                    "{tool}: no command names it and it is not listed in WITHOUT_COMMAND"
                )),
                (true, true) => Some(format!(
                    "{tool}: a command names it, so it leaves WITHOUT_COMMAND"
                )),
                _ => None,
            }
        })
        .collect()
}

/// [`problems`] against this server's tools.
#[must_use]
pub fn check(entries: &[(&str, &McpCoverage)]) -> Vec<String> {
    problems(entries, &RbMcpServer::tool_names())
}

/// Every entry of `pending` (tool, job id) whose job has no job file under
/// `<root>/docs/roadmap/jobs/` (archive included), or whose job is done - `Implemented`, or
/// `Partial (Abnahme offen ...)`, the code complete - while its tool is still pending.
#[must_use]
pub fn pending_job_problems(root: &Path, pending: &[(&str, &str)]) -> Vec<String> {
    pending
        .iter()
        .filter_map(|(tool, job)| match job_status(root, job) {
            None => Some(format!("{tool}: pending on {job}, which has no job file")),
            Some(status) if status == "Implemented" || status.starts_with("Partial (Abnahme") => {
                Some(format!(
                    "{tool}: pending on {job}, which is {status} - register the tool or name \
                     the job that brings it"
                ))
            }
            Some(_) => None,
        })
        .collect()
}

/// The status line of job `RB-<MMM>-<NN>`: `docs/roadmap/jobs/<MMM>-<NN>-*.md` or the same in
/// `archive/`; `None` without a file.
fn job_status(root: &Path, job: &str) -> Option<String> {
    let stem = format!("{}-", job.strip_prefix("RB-")?);
    let jobs = root.join("docs/roadmap/jobs");
    for dir in [jobs.clone(), jobs.join("archive")] {
        let Ok(files) = std::fs::read_dir(&dir) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            if !(name.starts_with(&stem) && name.ends_with(".md")) {
                continue;
            }
            let text = std::fs::read_to_string(file.path()).unwrap_or_default();
            let status = text
                .lines()
                .find_map(|line| line.trim().strip_prefix("- **Status:**"))
                .map(|status| status.trim().to_owned())
                .unwrap_or_default();
            return Some(status);
        }
    }
    None
}

#[cfg(test)]
#[path = "coverage_tests.rs"]
mod tests;
