//! `rbrowser screenshot` (RB-050-03): the arguments, the views they name and what is printed.
//! The call goes to the project's running instance ([`run`]: `Request::Screenshot` over the
//! instance channel), which captures with `crate::capture::screenshot` and answers the
//! [`ScreenshotAnswer`]; without `--json` the paths are printed one per line. No instance: exit
//! code 3; refused or no answer: 6; a view or the capture failed: 1.

use std::path::{Path, PathBuf};

use clap::Args;
use rb_engine::{Target, ViewId};

use super::plan::{Env, Refused, project_folder};
use super::{Cli, exit, paths};
use crate::capture::{CaptureMode, ScreenshotAnswer, ScreenshotArgs};
use crate::instance::client::{self, Outcome};
use crate::instance::protocol::{self, Refusal, Request};

/// The longest delay the command line takes, in seconds.
pub const MAX_DELAY_SECS: u64 = 60;

/// `rbrowser screenshot`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Args)]
pub struct ScreenshotCliArgs {
    /// The views, by breakpoint id or view id, comma-separated; every view when absent.
    #[arg(long, value_delimiter = ',', value_name = "ID")]
    pub views: Vec<String>,

    /// The whole page instead of the viewport.
    #[arg(long, conflicts_with = "element")]
    pub full: bool,

    /// One element, by CSS selector, with `capture.elementPadding` around it.
    #[arg(long, value_name = "SELECTOR")]
    pub element: Option<String>,

    /// One overview image of the views with label and size.
    #[arg(long, conflicts_with = "element")]
    pub overview: bool,

    /// Wait this many seconds first (at most 60).
    #[arg(long, value_name = "SECONDS", value_parser = clap::value_parser!(u64).range(0..=60))]
    pub delay: Option<u64>,

    /// Write to this folder instead of the project's screenshot folder.
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,

    /// Print the answer as JSON (paths, sizes, errors) instead of one path per line.
    #[arg(long)]
    pub json: bool,
}

impl ScreenshotCliArgs {
    /// The capture arguments: every view (narrowed by [`select_views`]), the mode, no delay
    /// unless asked, files.
    pub fn capture_args(&self) -> ScreenshotArgs {
        let mode = match (&self.element, self.full) {
            (Some(_), _) => CaptureMode::Element,
            (None, true) => CaptureMode::FullPage,
            (None, false) => CaptureMode::Viewport,
        };
        ScreenshotArgs {
            target: Some(Target::All),
            mode: Some(mode),
            selector: self.element.clone(),
            padding: None,
            overview: Some(self.overview),
            delay_ms: Some(self.delay.unwrap_or(0).min(MAX_DELAY_SECS) * 1000),
            save: Some(true),
            ..ScreenshotArgs::default()
        }
    }
}

/// The views `wanted` names among `views` (id and breakpoint, in layout order): a number is a
/// view id, anything else a breakpoint id (every view showing it). `None` when `wanted` is
/// empty (every view); an error names what matched nothing.
pub fn select_views(
    views: &[(ViewId, Option<String>)],
    wanted: &[String],
) -> Result<Option<Vec<ViewId>>, String> {
    if wanted.is_empty() {
        return Ok(None);
    }
    let mut chosen = Vec::new();
    for name in wanted {
        let name = name.trim();
        let matches: Vec<ViewId> = views
            .iter()
            .filter(|(id, breakpoint)| {
                name.parse::<u32>().is_ok_and(|number| id.0 == number)
                    || breakpoint.as_deref() == Some(name)
            })
            .map(|(id, _)| *id)
            .collect();
        if matches.is_empty() {
            return Err(format!("no view shows {name}"));
        }
        chosen.extend(matches);
    }
    Ok(Some(
        views
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| chosen.contains(id))
            .collect(),
    ))
}

/// What `rbrowser screenshot` prints: the answer as JSON, or the files one per line (the
/// overview last) and an `error:` line per view that failed.
pub fn render(answer: &ScreenshotAnswer, json: bool) -> String {
    if json {
        return serde_json::to_string_pretty(answer).unwrap_or_default();
    }
    let mut lines: Vec<String> = answer.paths().into_iter().map(str::to_owned).collect();
    for shot in &answer.shots {
        if let Some(error) = &shot.error {
            lines.push(format!("error: {}: {error}", shot.label));
        }
    }
    lines.join("\n")
}

/// The project of `rbrowser screenshot`: `--project` (or a folder argument), else the one found
/// upwards from the working folder - never the last project. No URL, no `--reload`.
pub fn project_root(cli: &Cli, env: &Env) -> Result<PathBuf, Refused> {
    if cli.reload || !cli.urls().is_empty() {
        return Err((
            exit::USAGE,
            "screenshot takes no URL and no --reload".to_owned(),
        ));
    }
    match cli.project_dir() {
        Some(dir) => project_folder(&dir.to_string_lossy(), env),
        None => crate::project::find_root(None, &env.cwd)
            .map(|root| paths::absolute(&root, &env.cwd))
            .ok_or_else(|| {
                (
                    exit::NO_PROJECT,
                    "screenshot needs a project: pass --project DIR or run it inside the project"
                        .to_owned(),
                )
            }),
    }
}

/// The request for the running instance: the capture arguments, the views as named (trimmed),
/// `--out` translated (WSL), made absolute against the working folder with `.` and `..`
/// resolved, and the whole held to the bounds the instance checks
/// ([`protocol::check_screenshot`]).
pub fn instance_request(args: &ScreenshotCliArgs, env: &Env) -> Result<Request, Refused> {
    let usage = |message: String| (exit::USAGE, message);
    let out = match &args.out {
        None => None,
        Some(dir) => {
            let text = dir
                .to_str()
                .ok_or_else(|| usage("--out: the folder name is not valid Unicode".to_owned()))?;
            let text = paths::translate(text, env.windows, env.distro.as_deref())
                .map_err(|error| usage(format!("--out {text}: {error}")))?;
            let absolute = paths::absolute(Path::new(&text), &env.cwd);
            Some(
                absolute
                    .to_str()
                    .ok_or_else(|| usage("--out: the folder name is not valid Unicode".to_owned()))?
                    .to_owned(),
            )
        }
    };
    let capture = args.capture_args();
    let views: Vec<String> = args
        .views
        .iter()
        .map(|view| view.trim().to_owned())
        .filter(|view| !view.is_empty())
        .collect();
    protocol::check_screenshot(&capture, &views, out.as_deref()).map_err(usage)?;
    Ok(Request::Screenshot {
        args: capture,
        views,
        out,
    })
}

/// What `rbrowser screenshot` ends with: the exit code, what goes to stdout and to stderr.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ending {
    /// The exit code.
    pub code: i32,
    /// The paths, or the answer as JSON.
    pub stdout: Option<String>,
    /// Why it did not succeed.
    pub stderr: Option<String>,
}

impl Ending {
    fn error(code: i32, message: String) -> Self {
        Self {
            code,
            stdout: None,
            stderr: Some(message),
        }
    }
}

/// The [`Ending`] of a hand-over to the instance of the project at `root`: the answer printed
/// (exit 0, or 1 when a view failed); no instance 3; a valid request that failed 1 with the
/// instance's reason; refused or no answer 6.
pub fn ending(outcome: Outcome, root: &Path, json: bool) -> Ending {
    let root = root.display();
    match outcome {
        Outcome::Answered(value) => match serde_json::from_value::<ScreenshotAnswer>(value) {
            Ok(answer) => {
                let failed = answer
                    .shots
                    .iter()
                    .chain(&answer.overview)
                    .any(|shot| shot.error.is_some());
                Ending {
                    code: if failed { exit::FAILED } else { exit::OK },
                    stdout: Some(render(&answer, json)),
                    stderr: None,
                }
            }
            Err(error) => Ending::error(
                exit::FAILED,
                format!("the running rBrowser for {root} answered no screenshots: {error}"),
            ),
        },
        Outcome::Handed => Ending::error(
            exit::FAILED,
            format!("the running rBrowser for {root} answered no screenshots"),
        ),
        Outcome::NotRunning => Ending::error(
            exit::NO_INSTANCE,
            format!("no rBrowser runs for {root}; start it with: rbrowser --project \"{root}\""),
        ),
        Outcome::Refused(Refusal::Failed, message) => Ending::error(
            exit::FAILED,
            message.unwrap_or_else(|| "the capture failed".to_owned()),
        ),
        Outcome::Refused(refusal, _) => Ending::error(
            exit::HANDOVER_FAILED,
            format!("the running rBrowser for {root} refused the call ({refusal:?})"),
        ),
        Outcome::Failed(error) => Ending::error(
            exit::HANDOVER_FAILED,
            format!("the running rBrowser for {root} did not answer: {error}"),
        ),
    }
}

/// `rbrowser screenshot`: the project, the request, the hand-over to the project's running
/// instance under `local`, the output; returns the exit code.
pub fn run(cli: &Cli, args: &ScreenshotCliArgs, env: &Env, local: Option<&Path>) -> i32 {
    let prepared = project_root(cli, env)
        .and_then(|root| instance_request(args, env).map(|request| (request, root)));
    let (request, root) = match prepared {
        Ok(prepared) => prepared,
        Err((code, message)) => {
            tracing::error!("screenshot: {message}");
            eprintln!("rbrowser screenshot: {message}");
            return code;
        }
    };
    let outcome = local.map_or(Outcome::NotRunning, |local| {
        client::hand_over(local, &root, request)
    });
    let ending = ending(outcome, &root, args.json);
    if let Some(text) = &ending.stdout {
        println!("{text}");
    }
    if let Some(text) = &ending.stderr {
        tracing::error!(code = ending.code, "screenshot: {text}");
        eprintln!("rbrowser screenshot: {text}");
    }
    ending.code
}

#[cfg(test)]
#[path = "screenshot_tests.rs"]
mod tests;
