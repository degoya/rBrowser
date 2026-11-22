//! The command line of the browser process (RB-010-11). CEF's child processes get their own
//! (`--type=...`) and never reach this parser.
//!
//! `rbrowser [URL|DIR]...` and `rbrowser open [URL|DIR]...` open a project (`--project DIR`, a
//! folder argument, else the folder found upwards from the working folder) and load the first
//! URL in every view; a second call for a project that is open already hands its URL and flags
//! to that instance and ends ([`plan`], `crate::instance`). `--reload` reloads the running
//! instance, `--background` opens without taking the focus, `init` writes the config
//! ([`init`]), `diagnostics` the diagnostics ZIP (RB-010-10), `screenshot` captures the views
//! of the running instance ([`screenshot`], RB-050-03), `capture sitemap` a site's pages in
//! every breakpoint ([`capture`], RB-050-05), `engines firefox --list` the installed Firefox
//! editions ([`engines`], RB-080-32), `check` the check plugins in the running instance with the
//! findings exported and an exit code from them ([`check`], RB-092-05).

pub mod capture;
pub mod check;
pub mod console;
pub mod engines;
pub mod init;
pub mod paths;
pub mod plan;
pub mod screenshot;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

pub use plan::{Env, Plan, before_ui};

/// The version line: the workspace version, the commit and the build time from build.rs.
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("RB_BUILD_COMMIT"),
    ", built ",
    env!("RB_BUILD_TIME"),
    ")"
);

/// The exit codes of `rbrowser` (also in `--help`).
pub mod exit {
    /// Done: the window was closed, the call was handed over, the config was written.
    pub const OK: i32 = 0;
    /// Something failed (the browser did not start, a file could not be written, a screenshot
    /// of a view failed).
    pub const FAILED: i32 = 1;
    /// The command line is wrong (clap's own code), or rBrowser was started from the WSL share.
    pub const USAGE: i32 = 2;
    /// `--reload` or `screenshot`, but no rBrowser runs for the project.
    pub const NO_INSTANCE: i32 = 3;
    /// The project folder does not exist, or no project was given or found.
    pub const NO_PROJECT: i32 = 4;
    /// `init`: the project has a config already.
    pub const CONFIG_EXISTS: i32 = 5;
    /// The running rBrowser of the project refused the call or did not answer.
    pub const HANDOVER_FAILED: i32 = 6;
    /// `init`: not confirmed (no `--yes` and no "y" on the terminal).
    pub const NOT_CONFIRMED: i32 = 7;
}

/// The exit codes as `--help` lists them.
const EXIT_HELP: &str = "\
Exit codes:
  0  done (or handed over to the project's running rBrowser)
  1  failed (screenshot: a view or the capture failed)
  2  wrong command line, or started from the WSL share
  3  --reload, screenshot: no rBrowser runs for the project
  4  the project folder does not exist, or no project was found
  5  init: the project has a config already
  6  the running rBrowser refused the call or did not answer
  7  init: not confirmed

check has its own: 0 no finding at or above --fail-on, 1 one or more, 2 the run could not be
judged (a plugin failed, the run was cancelled, no rBrowser runs for the project, the call
failed or the command line is wrong).";

/// rBrowser, a development browser that shows every breakpoint of a site at once.
#[derive(Debug, Parser)]
#[command(name = "rbrowser", version = VERSION, about, after_long_help = EXIT_HELP)]
pub struct Cli {
    /// The project folder (the one holding `.rbrowser/config.json`); found from the working
    /// folder upwards when left out. A Linux path from WSL is translated to the WSL share.
    #[arg(long, global = true, value_name = "DIR")]
    pub project: Option<PathBuf>,

    /// Load this URL instead of the project's `startUrl`.
    #[arg(long, value_name = "URL")]
    pub url: Option<String>,

    /// Reload every view of the project's running rBrowser (for build scripts); fails with
    /// exit code 3 when none runs.
    #[arg(long, global = true)]
    pub reload: bool,

    /// Open the window, or hand the URL over, without taking the focus.
    #[arg(long, global = true)]
    pub background: bool,

    /// Run without the Chromium sandbox (the fallback of ADR 0001, Decision 6).
    #[arg(long, global = true)]
    pub no_sandbox: bool,

    /// Install every plugin that ships with rBrowser at this start, not only the chosen ones
    /// (for tests and team rollouts).
    #[arg(long)]
    pub install_all_bundled_plugins: bool,

    /// URLs to load (the first one in every view); a folder (`.`, `C:\site`) names the project.
    #[arg(value_name = "URL|DIR")]
    pub targets: Vec<String>,

    /// What to do; without a command the project opens.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Open a project and load URLs - the same as without a command.
    Open(OpenArgs),
    /// Write `.rbrowser/config.json` and `.rbrowser/.gitignore` for the project folder
    /// (`--project`, else the working folder); an existing config is never written over.
    Init(InitArgs),
    /// Write the diagnostics ZIP (logs, crash dumps, versions, the masked config) and print its
    /// path. Nothing is sent anywhere.
    Diagnostics(rb_diag::DiagnosticsArgs),
    /// Capture the views of the project's running rBrowser as PNG files and print their paths
    /// (`--json`: the whole answer); exit code 3 when none runs, 1 when a view failed.
    Screenshot(screenshot::ScreenshotCliArgs),
    /// Speak MCP on stdin and stdout, forwarded to the project's running rBrowser (its MCP
    /// server must be on); fails with exit code 3 when none runs. For `claude mcp add`.
    /// `mcp tools --format anthropic` prints the core tools' definitions instead (RB-098-01).
    Mcp(crate::mcp::cli::McpCliArgs),
    /// Tunnel rBrowser's CDP proxy into WSL in NAT mode: `rbrowser-wsl cdp-proxy` listens on
    /// 127.0.0.1 in the distribution until it ends (RB-030-06).
    CdpTunnel(CdpTunnelArgs),
    /// The project's flows (`.rbrowser/flows/*.ts`): `flow list`, and `flow run <name>` in the
    /// running rBrowser's views - exit code 1 when a view failed (RB-040-07).
    Flow(crate::flows::cli::FlowArgs),
    /// `capture sitemap`: every page of the site's sitemap in every breakpoint, in the running
    /// rBrowser's hidden views - exit code 1 when a shot failed or the run was cancelled
    /// (RB-050-05).
    Capture(capture::CaptureCliArgs),
    /// `engines firefox --list`: the installed Firefox editions (kind, version, path) in the
    /// order rBrowser searches them, and the configured path; no running rBrowser needed
    /// (RB-080-32).
    Engines(engines::EnginesCliArgs),
    /// Run the check plugins in the project's running rBrowser over every view and write the
    /// findings as SARIF, JUnit, HTML or JSON; prints the file. Exit code 0 without a finding at
    /// or above --fail-on, 1 with one, 2 when a plugin failed or no rBrowser runs (RB-092-05).
    Check(check::CheckCliArgs),
}

/// `rbrowser cdp-tunnel`.
#[derive(Debug, Args)]
pub struct CdpTunnelArgs {
    /// The CDP proxy's port on Windows (as `cdp_endpoint` names it); `cdp.port` when absent.
    #[arg(long)]
    pub port: Option<u16>,

    /// The WSL distribution; `wsl.distro`, else WSL's default, when absent.
    #[arg(long)]
    pub distro: Option<String>,
}

/// `rbrowser open`.
#[derive(Debug, Args)]
pub struct OpenArgs {
    /// URLs to load (the first one in every view); a folder (`.`, `C:\site`) names the project.
    #[arg(value_name = "URL|DIR")]
    pub targets: Vec<String>,
}

/// `rbrowser init`.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// The site's URL: the analyser reads its breakpoints and it becomes the `startUrl`.
    #[arg(long = "url", value_name = "URL")]
    pub start_url: Option<String>,

    /// Write without asking.
    #[arg(long, short = 'y')]
    pub yes: bool,
}

impl Cli {
    /// The positional arguments, of `rbrowser` and of `rbrowser open`.
    pub fn targets(&self) -> impl Iterator<Item = &str> {
        let open = match &self.command {
            Some(Command::Open(args)) => args.targets.as_slice(),
            _ => &[],
        };
        self.targets.iter().chain(open).map(String::as_str)
    }

    /// The project folder asked for, as written: `--project DIR` or a folder argument.
    pub fn project_dir(&self) -> Option<PathBuf> {
        self.project.clone().or_else(|| {
            self.targets()
                .find(|target| paths::looks_like_path(target))
                .map(PathBuf::from)
        })
    }

    /// The URLs asked for, as written: `--url`, then the arguments that are no folder.
    pub fn urls(&self) -> Vec<&str> {
        self.url
            .as_deref()
            .into_iter()
            .chain(
                self.targets()
                    .filter(|target| !paths::looks_like_path(target)),
            )
            .collect()
    }

    /// The URL every view loads instead of the project's `startUrl`: the first one asked for,
    /// completed and checked.
    pub fn start_url(&self) -> Option<String> {
        self.urls()
            .first()
            .and_then(|url| crate::instance::protocol::open_url(url).ok())
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
