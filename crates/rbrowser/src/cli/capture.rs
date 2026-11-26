//! `rbrowser capture sitemap [<url>] [--breakpoints a,b | --visible] [--env staging] [--max N]
//! [--compare] [--json]` (RB-050-05, `--visible` RB-099-08): the capture runs in the project's
//! running rBrowser, in hidden render views. The call is handed over the instance channel
//! (`Request::SitemapCapture`), which answers once the pages are found; the command line then
//! follows the run's `run.json`, prints its progress to stderr and ends with the gallery's path
//! (`--json`: the run's status). Ctrl+C ends only the command line: the run goes on in rBrowser
//! (`sitemap_cancel`, `Leader C X`).
//!
//! `--list` (RB-099-09) finds the pages without capturing one and prints them, one URL a line
//! (`--json`: the discovery with its tree); `--urls <file>` captures exactly the URLs of a file
//! (one a line, `#` comments), each one the source's discovery finds - the rest is refused
//! (`super::capture_list`).
//!
//! Exit codes: 0 every shot taken, 1 a shot failed, the run was cancelled or it stalled, 2 a
//! wrong command line, 3 no rBrowser runs for the project, 4 no project, 6 refused.

use std::path::Path;
use std::time::{Duration, Instant};

use clap::{Args, Subcommand};
use rb_visual::sitemap::{RunState, SitemapRun};
use rb_visual::sitemap_store::RunStore;

use super::plan::{Env, Refused, project_folder};
use super::{Cli, exit};
use crate::instance::client::{self, Outcome};
use crate::instance::protocol::{Refusal, Request};
use crate::sitemap::status::status_of;
use crate::sitemap::tool::{CaptureArgs, CaptureStarted, CaptureViews};

/// How often `run.json` is read.
const POLL: Duration = Duration::from_secs(1);
/// How long a run may make no progress before the command line gives up on it.
pub const STALL: Duration = Duration::from_secs(15 * 60);

/// `rbrowser capture`.
#[derive(Debug, Args)]
pub struct CaptureCliArgs {
    /// What to capture.
    #[command(subcommand)]
    pub command: CaptureCommand,
}

/// The subcommands of `rbrowser capture`.
#[derive(Debug, Subcommand)]
pub enum CaptureCommand {
    /// Capture every page of the site's sitemap in every breakpoint, in the project's running
    /// rBrowser (hidden views; the grid stays usable), into
    /// `.rbrowser/out/sitemap-capture/<run>/` with `run.json` and the gallery `index.html`.
    Sitemap(SitemapCliArgs),
}

/// `rbrowser capture sitemap`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Args)]
pub struct SitemapCliArgs {
    /// A site (its sitemaps are found), a sitemap URL, or a project-relative .xml, .xml.gz or
    /// .txt file; the project's startUrl when absent.
    #[arg(value_name = "URL|FILE")]
    pub url: Option<String>,

    /// The breakpoints by id, comma-separated; every Chromium breakpoint when absent.
    #[arg(long, value_delimiter = ',', value_name = "ID")]
    pub breakpoints: Vec<String>,

    /// Only the breakpoints the grid shows when the capture starts (RB-099-08).
    #[arg(long, conflicts_with = "breakpoints")]
    pub visible: bool,

    /// An environment id: the sitemap's URLs move to its origin, the path kept.
    #[arg(long, value_name = "ID")]
    pub env: Option<String>,

    /// At most this many pages.
    #[arg(long, value_name = "N")]
    pub max: Option<usize>,

    /// Only pages matching one of these globs over the path (or `re:<regex>`), comma-separated.
    #[arg(long, value_delimiter = ',', value_name = "PATTERN")]
    pub include: Vec<String>,

    /// Leave out pages matching one of these patterns, comma-separated.
    #[arg(long, value_delimiter = ',', value_name = "PATTERN")]
    pub exclude: Vec<String>,

    /// Compare each shot with its visual-regression baseline.
    #[arg(long)]
    pub compare: bool,

    /// The project bookmarks instead of a sitemap.
    #[arg(long, conflicts_with = "url")]
    pub bookmarks: bool,

    /// Continue this run: what is done stays, failed shots are tried again.
    #[arg(long, value_name = "RUN")]
    pub resume: Option<String>,

    /// The session every page is captured in; each breakpoint's own when absent.
    #[arg(long, value_name = "ID")]
    pub session: Option<String>,

    /// Print the run's status as JSON at the end instead of the gallery's path.
    #[arg(long)]
    pub json: bool,

    /// Only find the pages and print their URLs, one a line; nothing is captured.
    #[arg(long, conflicts_with_all = [
        "resume", "urls", "compare", "breakpoints", "session", "env", "visible"
    ])]
    pub list: bool,

    /// Capture exactly the URLs of this `.txt` file inside the project (one a line, `#`
    /// comments, at most 100000), each one the source's discovery finds.
    #[arg(long, value_name = "FILE", conflicts_with = "resume")]
    pub urls: Option<std::path::PathBuf>,
}

impl SitemapCliArgs {
    /// The arguments of the capture, checked as the instance checks them.
    pub fn capture_args(&self) -> Result<CaptureArgs, String> {
        let list = |items: &[String]| (!items.is_empty()).then(|| items.to_vec());
        let args = CaptureArgs {
            url: self.url.clone(),
            bookmarks: self.bookmarks,
            breakpoints: self.breakpoints.clone(),
            env: self.env.clone(),
            max: self.max,
            include: list(&self.include),
            exclude: list(&self.exclude),
            compare: self.compare,
            resume: self.resume.clone(),
            session: self.session.clone(),
            views: self.visible.then_some(CaptureViews::Visible),
            ..CaptureArgs::default()
        };
        args.check()?;
        Ok(args)
    }
}

/// The project: `--project` (or a folder argument), else the one found upwards from the
/// working folder.
fn project_root(cli: &Cli, env: &Env) -> Result<std::path::PathBuf, Refused> {
    match cli.project_dir() {
        Some(dir) => project_folder(&dir.to_string_lossy(), env),
        None => crate::project::find_root(None, &env.cwd)
            .map(|root| super::paths::absolute(&root, &env.cwd))
            .ok_or_else(|| {
                (
                    exit::NO_PROJECT,
                    "capture needs a project: pass --project DIR or run it inside the project"
                        .to_owned(),
                )
            }),
    }
}

/// One progress line of `run`.
pub fn progress_line(run: &SitemapRun) -> String {
    let progress = run.progress(1);
    let left = progress
        .eta_seconds
        .map(|seconds| format!(", about {seconds} s left"))
        .unwrap_or_default();
    format!(
        "sitemap {}: {}/{} done, {} failed{left}",
        run.run_id, progress.done, progress.total, progress.failed
    )
}

/// The exit code of a run that ended as `run`.
pub fn exit_code(run: &SitemapRun) -> i32 {
    let progress = run.progress(1);
    if run.state == RunState::Finished && progress.failed == 0 {
        exit::OK
    } else {
        exit::FAILED
    }
}

/// What the hand-over gave: the run started, or the exit code and why not.
pub fn started(outcome: Outcome, root: &Path) -> Result<CaptureStarted, Refused> {
    answered(outcome, root, "run")
}

/// What the hand-over gave: its answer as `T` (`what` names it), or the exit code and why not.
pub fn answered<T: serde::de::DeserializeOwned>(
    outcome: Outcome,
    root: &Path,
    what: &str,
) -> Result<T, Refused> {
    let root = root.display();
    match outcome {
        Outcome::Answered(value) => serde_json::from_value(value).map_err(|error| {
            (
                exit::FAILED,
                format!("the running rBrowser for {root} answered no {what}: {error}"),
            )
        }),
        Outcome::Handed => Err((
            exit::FAILED,
            format!("the running rBrowser for {root} answered no {what}"),
        )),
        Outcome::NotRunning => Err((
            exit::NO_INSTANCE,
            format!("no rBrowser runs for {root}; start it with: rbrowser --project \"{root}\""),
        )),
        Outcome::Refused(Refusal::Failed, message) => Err((
            exit::FAILED,
            message.unwrap_or_else(|| "the capture did not start".to_owned()),
        )),
        Outcome::Refused(refusal, _) => Err((
            exit::HANDOVER_FAILED,
            format!("the running rBrowser for {root} refused the call ({refusal:?})"),
        )),
        Outcome::Failed(error) => Err((
            exit::HANDOVER_FAILED,
            format!("the running rBrowser for {root} did not answer: {error}"),
        )),
    }
}

/// Reads `run.json` of `run_id` until the run ended (or made no progress for [`STALL`]),
/// printing a line whenever it moved; the run as it ended.
fn follow(root: &Path, run_id: &str) -> Result<SitemapRun, String> {
    let store = RunStore::open(root, run_id, false)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("the run {run_id} has no folder"))?;
    let mut last = String::new();
    let mut moved = Instant::now();
    loop {
        if let Some(run) = store.load().map_err(|error| error.to_string())? {
            let line = progress_line(&run);
            if line != last {
                eprintln!("{line}");
                last = line;
                moved = Instant::now();
            }
            if run.state != RunState::Running {
                return Ok(run);
            }
        }
        if moved.elapsed() > STALL {
            return Err(format!(
                "the run {run_id} made no progress for {} minutes",
                STALL.as_secs() / 60
            ));
        }
        std::thread::sleep(POLL);
    }
}

/// `rbrowser capture sitemap`: the project, the hand-over, the run followed; the exit code.
pub fn run(cli: &Cli, args: &CaptureCliArgs, env: &Env, local: Option<&Path>) -> i32 {
    let CaptureCommand::Sitemap(args) = &args.command;
    let fail = |code: i32, message: &str| {
        tracing::error!(code, "capture sitemap: {message}");
        eprintln!("rbrowser capture sitemap: {message}");
        code
    };
    let root = match project_root(cli, env) {
        Ok(root) => root,
        Err((code, message)) => return fail(code, &message),
    };
    if args.list {
        return super::capture_list::list(args, &root, local);
    }
    let capture = match args.capture_args().and_then(|capture| {
        super::capture_list::with_urls_file(capture, args.urls.as_deref(), &env.cwd, &root)
    }) {
        Ok(capture) => capture,
        Err(message) => return fail(exit::USAGE, &message),
    };
    let outcome = local.map_or(Outcome::NotRunning, |local| {
        client::hand_over(local, &root, Request::SitemapCapture { args: capture })
    });
    let started = match started(outcome, &root) {
        Ok(started) => started,
        Err((code, message)) => return fail(code, &message),
    };
    eprintln!(
        "sitemap {}: {} shots, in {}",
        started.run_id, started.pending, started.folder
    );
    let run = match follow(&root, &started.run_id) {
        Ok(run) => run,
        Err(message) => return fail(exit::FAILED, &message),
    };
    let dir = Path::new(&started.folder);
    let status = status_of(&run, dir, 1);
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&status).unwrap_or_default()
        );
    } else if let Some(gallery) = &status.gallery {
        println!("{gallery}");
    }
    if run.state == RunState::Cancelled {
        eprintln!(
            "rbrowser capture sitemap: cancelled; continue with --resume {}",
            run.run_id
        );
    }
    exit_code(&run)
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
