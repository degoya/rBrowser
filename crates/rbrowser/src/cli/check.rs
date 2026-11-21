//! `rbrowser check` (RB-092-05): runs the check plugins in the project's running rBrowser over
//! every view, writes the run as SARIF, JUnit, HTML or JSON (`--format`, else
//! `checks.export.defaultFormat`) to `--out` or the project's export folder, prints the file's
//! path and a line of counts, and ends with an exit code a pipeline can read: 0 without a
//! finding at or above `--fail-on` (else `checks.export.failOn`, else `error`), 1 with one, 2
//! when the run could not be judged - a plugin failed, the run was cancelled, no rBrowser runs
//! for the project, or the call failed. The run is the window's own: the findings panel shows it
//! and `.rbrowser/out/checks/` keeps it.

use std::path::{Path, PathBuf};

use clap::Args;

use super::plan::{Env, project_folder};
use super::screenshot::Ending;
use super::{Cli, paths};
use crate::checks::export::check::{CheckAnswer, CheckRequest};
use crate::checks::export::{self, FindingsFailOn};
use crate::instance::client::{self, Outcome};
use crate::instance::protocol::{Refusal, Request};

/// The exit codes of `rbrowser check`.
pub mod code {
    /// No finding at or above the threshold.
    pub const CLEAN: i32 = 0;
    /// A finding at or above the threshold.
    pub const FINDINGS: i32 = 1;
    /// The run could not be judged: a plugin failed, no instance runs, the call failed.
    pub const ERROR: i32 = 2;
}

/// `rbrowser check`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Args)]
pub struct CheckCliArgs {
    /// Only these check plugins, by id, comma-separated; every enabled one when absent.
    #[arg(long, value_delimiter = ',', value_name = "ID")]
    pub plugins: Vec<String>,

    /// The file format; the setting checks.export.defaultFormat when absent.
    #[arg(long, value_parser = ["sarif", "junit", "html", "json"])]
    pub format: Option<String>,

    /// Write the file here; the project's export folder (.rbrowser/out/exports) when absent.
    #[arg(long, value_name = "FILE")]
    pub out: Option<PathBuf>,

    /// From which severity on a finding fails the call (exit code 1); the setting
    /// checks.export.failOn when absent.
    #[arg(long, value_parser = ["error", "warning", "info"])]
    pub fail_on: Option<String>,
}

fn fail_on_named(name: &str) -> Option<FindingsFailOn> {
    match name {
        "error" => Some(FindingsFailOn::Error),
        "warning" => Some(FindingsFailOn::Warning),
        "info" => Some(FindingsFailOn::Info),
        _ => None,
    }
}

/// The project of `rbrowser check`: `--project` (or a folder argument), else the one found
/// upwards from the working folder. No URL, no `--reload`.
///
/// # Errors
///
/// Why not, for the person at the command line.
pub fn project_root(cli: &Cli, env: &Env) -> Result<PathBuf, String> {
    if cli.reload || !cli.urls().is_empty() {
        return Err("check takes no URL and no --reload".to_owned());
    }
    match cli.project_dir() {
        Some(dir) => project_folder(&dir.to_string_lossy(), env).map_err(|(_, message)| message),
        None => crate::project::find_root(None, &env.cwd)
            .map(|root| paths::absolute(&root, &env.cwd))
            .ok_or_else(|| {
                "check needs a project: pass --project DIR or run it inside the project".to_owned()
            }),
    }
}

/// The request for the running instance: the plugins as named (trimmed), the format and the
/// threshold, `--out` translated (WSL) and made absolute against the working folder, the whole
/// held to the bounds the instance checks ([`CheckRequest::check`]).
///
/// # Errors
///
/// Why not, for the person at the command line.
pub fn instance_request(args: &CheckCliArgs, env: &Env) -> Result<Request, String> {
    let out = match &args.out {
        None => None,
        Some(file) => {
            let text = file
                .to_str()
                .ok_or_else(|| "--out: the file name is not valid Unicode".to_owned())?;
            if text.ends_with(['/', '\\']) {
                return Err(format!("{text}: --out names a file, not a folder"));
            }
            let text = paths::translate(text, env.windows, env.distro.as_deref())
                .map_err(|error| format!("--out {text}: {error}"))?;
            let absolute = paths::absolute(Path::new(&text), &env.cwd);
            Some(
                absolute
                    .to_str()
                    .ok_or_else(|| "--out: the file name is not valid Unicode".to_owned())?
                    .to_owned(),
            )
        }
    };
    let request = CheckRequest {
        plugins: args
            .plugins
            .iter()
            .map(|plugin| plugin.trim().to_owned())
            .filter(|plugin| !plugin.is_empty())
            .collect(),
        format: args.format.as_deref().and_then(export::parse_format),
        out,
        fail_on: args.fail_on.as_deref().and_then(fail_on_named),
    };
    request.check()?;
    Ok(Request::Check { args: request })
}

/// The line of counts after the path.
fn counts(answer: &CheckAnswer) -> String {
    let summary = &answer.summary;
    format!(
        "{} findings: {} errors, {} warnings, {} infos (fail on {})",
        summary.total,
        summary.error,
        summary.warning,
        summary.info,
        match answer.fail_on {
            FindingsFailOn::Error => "error",
            FindingsFailOn::Warning => "warning",
            FindingsFailOn::Info => "info",
        }
    )
}

fn error(message: String) -> Ending {
    Ending {
        code: code::ERROR,
        stdout: None,
        stderr: Some(message),
    }
}

/// The [`Ending`] of a hand-over to the instance of the project at `root`: the path printed and
/// the exit code from the answer; anything but an answer is 2 with the reason.
pub fn ending(outcome: Outcome, root: &Path) -> Ending {
    let root = root.display();
    let value = match outcome {
        Outcome::Answered(value) => value,
        Outcome::NotRunning => {
            return error(format!(
                "no rBrowser runs for {root}; start it with: rbrowser --project \"{root}\", then run the check again"
            ));
        }
        Outcome::Handed => {
            return error(format!(
                "the running rBrowser for {root} answered no check run"
            ));
        }
        Outcome::Refused(Refusal::Failed, message) => {
            return error(message.unwrap_or_else(|| "the check run failed".to_owned()));
        }
        Outcome::Refused(refusal, _) => {
            return error(format!(
                "the running rBrowser for {root} refused the call ({refusal:?})"
            ));
        }
        Outcome::Failed(reason) => {
            return error(format!(
                "the running rBrowser for {root} did not answer: {reason}"
            ));
        }
    };
    let answer = match serde_json::from_value::<CheckAnswer>(value) {
        Ok(answer) => answer,
        Err(reason) => {
            return error(format!(
                "the running rBrowser for {root} answered no check run: {reason}"
            ));
        }
    };
    let mut stderr = counts(&answer);
    let code = if answer.cancelled {
        stderr.push_str("\nthe check run was cancelled before it was through");
        code::ERROR
    } else if !answer.failed_plugins.is_empty() {
        stderr.push_str(&format!(
            "\nthese plugins failed, so the run is incomplete: {}",
            answer.failed_plugins.join(", ")
        ));
        code::ERROR
    } else if export::fail_on(answer.fail_on).fails(&answer.summary) {
        code::FINDINGS
    } else {
        code::CLEAN
    };
    Ending {
        code,
        stdout: Some(answer.path),
        stderr: Some(stderr),
    }
}

/// `rbrowser check`: the project, the request, the hand-over to the project's running instance
/// under `local`, the output; returns the exit code.
pub fn run(cli: &Cli, args: &CheckCliArgs, env: &Env, local: Option<&Path>) -> i32 {
    let prepared = project_root(cli, env)
        .and_then(|root| instance_request(args, env).map(|request| (request, root)));
    let (request, root) = match prepared {
        Ok(prepared) => prepared,
        Err(message) => {
            tracing::error!("check: {message}");
            eprintln!("rbrowser check: {message}");
            return code::ERROR;
        }
    };
    let outcome = local.map_or(Outcome::NotRunning, |local| {
        client::hand_over(local, &root, request)
    });
    let ending = ending(outcome, &root);
    if let Some(text) = &ending.stdout {
        println!("{text}");
    }
    if let Some(text) = &ending.stderr {
        if ending.code == code::ERROR {
            tracing::error!(code = ending.code, "check: {text}");
        }
        eprintln!("rbrowser check: {text}");
    }
    ending.code
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
