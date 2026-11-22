//! What a launch asks for, checked before any window exists ([`plan`]), and the hand-over to
//! the project's running instance ([`forward`]): a second call for an open project passes its
//! URL and flags there and ends with 0; `--reload` without a running instance ends with
//! [`exit::NO_INSTANCE`].

use std::path::{Path, PathBuf};

use rb_config::UserSettings;

use super::{Cli, Command, exit, init, paths, screenshot};
use crate::instance::client::{self, Outcome};
use crate::instance::protocol::{self, Request};

/// What a launch knows about its surroundings.
#[derive(Clone, Debug)]
pub struct Env {
    /// The working folder.
    pub cwd: PathBuf,
    /// Whether paths are read the Windows way (Linux paths from WSL are translated).
    pub windows: bool,
    /// The WSL distribution of the caller, when started from WSL.
    pub distro: Option<String>,
    /// `recentProjects`.
    pub recent: Vec<String>,
    /// `startup.openLastProject`.
    pub open_last_project: bool,
    /// `startup.focusOnOpen`.
    pub focus_on_open: bool,
}

impl Env {
    /// This process in the working folder `cwd` with the user settings `user` (read once by the
    /// launch): the WSL distribution from the working folder on the share or
    /// `WSL_DISTRO_NAME`.
    pub fn from_user(cwd: &Path, user: &UserSettings) -> Self {
        let distro = paths::distro_from_cwd(&cwd.to_string_lossy()).or_else(|| {
            std::env::var("WSL_DISTRO_NAME")
                .ok()
                .filter(|name| paths::valid_distro(name))
        });
        Self {
            cwd: cwd.to_path_buf(),
            windows: cfg!(windows),
            distro,
            recent: user.recent_projects.clone(),
            open_last_project: user.startup.open_last_project,
            focus_on_open: user.startup.focus_on_open,
        }
    }
}

/// A launch, checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The project root, absolute.
    pub root: Option<PathBuf>,
    /// The URLs, completed and checked; the first is loaded.
    pub urls: Vec<String>,
    /// `--reload`.
    pub reload: bool,
    /// Without taking the focus.
    pub background: bool,
}

/// A refused launch: the exit code and the message.
pub type Refused = (i32, String);

/// Checks `cli` in `env`: the project (given, found upwards, or the last one when the setting
/// asks for it), the URLs and the flags.
pub fn plan(cli: &Cli, env: &Env) -> Result<Plan, Refused> {
    let folders: Vec<&str> = cli
        .targets()
        .filter(|target| paths::looks_like_path(target))
        .collect();
    if folders.len() + usize::from(cli.project.is_some()) > 1 {
        return Err((exit::USAGE, "more than one project folder given".to_owned()));
    }
    let given = cli
        .project
        .as_ref()
        .map(|dir| dir.to_string_lossy().into_owned())
        .or_else(|| folders.first().map(|dir| (*dir).to_owned()));
    let urls = cli
        .urls()
        .into_iter()
        .map(protocol::open_url)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|message| (exit::USAGE, message))?;
    if cli.reload && !urls.is_empty() {
        return Err((exit::USAGE, "--reload takes no URL".to_owned()));
    }
    let root = match given {
        Some(dir) => Some(project_folder(&dir, env)?),
        None => crate::project::find_root(None, &env.cwd)
            .map(|root| paths::absolute(&root, &env.cwd))
            .or_else(|| {
                (env.open_last_project && !cli.reload)
                    .then(|| last_project(&env.recent))
                    .flatten()
            }),
    };
    if cli.reload && root.is_none() {
        return Err((
            exit::NO_PROJECT,
            "--reload needs a project: pass --project DIR or run it inside the project".to_owned(),
        ));
    }
    Ok(Plan {
        root,
        urls,
        reload: cli.reload,
        background: cli.background || !env.focus_on_open,
    })
}

/// The folder `dir` as written on the command line: translated, absolute, existing.
pub(super) fn project_folder(dir: &str, env: &Env) -> Result<PathBuf, Refused> {
    let text = paths::translate(dir, env.windows, env.distro.as_deref())
        .map_err(|error| (exit::USAGE, format!("{dir}: {error}")))?;
    let root = paths::absolute(Path::new(&text), &env.cwd);
    if root.is_dir() {
        Ok(root)
    } else {
        Err((
            exit::NO_PROJECT,
            format!("{} is not a folder", root.display()),
        ))
    }
}

/// The newest recent project that still has its config.
fn last_project(recent: &[String]) -> Option<PathBuf> {
    recent.iter().map(PathBuf::from).find(|root| {
        root.join(rb_config::CONFIG_DIR)
            .join(rb_config::CONFIG_FILE)
            .is_file()
    })
}

/// Hands `plan` to the running instance of its project under `local`: `Some(exit code)` ends
/// this process, `None` starts the browser.
pub fn forward(plan: &Plan, local: Option<&Path>) -> Option<i32> {
    let root = plan.root.as_deref()?;
    let request = if plan.reload {
        Request::Reload {
            ignore_cache: false,
        }
    } else {
        Request::Open {
            urls: plan.urls.clone(),
            background: plan.background,
        }
    };
    let outcome = local.map_or(Outcome::NotRunning, |local| {
        client::hand_over(local, root, request)
    });
    match outcome {
        Outcome::Handed | Outcome::Answered(_) => {
            tracing::info!(project = %root.display(), "handed over to the running instance");
            Some(exit::OK)
        }
        Outcome::NotRunning if plan.reload => {
            let message = format!(
                "no rBrowser runs for {}; start it with: rbrowser --project \"{}\"",
                root.display(),
                root.display()
            );
            tracing::error!("--reload: {message}");
            eprintln!("rbrowser --reload: {message}");
            Some(exit::NO_INSTANCE)
        }
        Outcome::NotRunning => None,
        Outcome::Refused(refusal, message) => {
            tracing::error!(
                ?refusal,
                ?message,
                project = %root.display(),
                "the running instance refused the call"
            );
            eprintln!(
                "rbrowser: the running rBrowser for {} refused the call ({})",
                root.display(),
                message.unwrap_or_else(|| format!("{refusal:?}"))
            );
            Some(exit::HANDOVER_FAILED)
        }
        Outcome::Failed(error) => {
            tracing::error!(%error, project = %root.display(), "the running instance did not answer");
            eprintln!(
                "rbrowser: the running rBrowser for {} did not answer: {error}",
                root.display()
            );
            Some(exit::HANDOVER_FAILED)
        }
    }
}

/// What the command line does before any window exists (contract 0.1-w1b): `init` writes the
/// config, `screenshot`, `--reload` and a second call for a running project hand over to its
/// instance; `mcp` forwards stdio to the running instance (RB-030-01).
/// `user` are the user settings the launch read once. `Err(exit code)` ends the process;
/// `Ok(Some(plan))` goes on to start the browser with that plan (the launch reuses it),
/// `Ok(None)` without one (`diagnostics`, which the launch runs itself).
pub fn before_ui(cli: &Cli, user: &UserSettings) -> Result<Option<Plan>, i32> {
    let env = Env::from_user(&std::env::current_dir().unwrap_or_default(), user);
    if let Some(Command::Init(args)) = &cli.command {
        let root = match cli.project.as_ref() {
            Some(dir) => {
                paths::translate(&dir.to_string_lossy(), env.windows, env.distro.as_deref())
                    .map(|text| paths::absolute(Path::new(&text), &env.cwd))
            }
            None => Ok(env.cwd.clone()),
        };
        return Err(match root {
            Ok(root) => init::run(&root, args.start_url.as_deref(), args.yes),
            Err(error) => {
                eprintln!("rbrowser init: {error}");
                exit::USAGE
            }
        });
    }
    if let Some(Command::Screenshot(args)) = &cli.command {
        return Err(screenshot::run(
            cli,
            args,
            &env,
            rb_config::local_dir().as_deref(),
        ));
    }
    // `rbrowser capture sitemap` runs in the project's running instance (RB-050-05).
    if let Some(Command::Capture(args)) = &cli.command {
        return Err(super::capture::run(
            cli,
            args,
            &env,
            rb_config::local_dir().as_deref(),
        ));
    }
    // `rbrowser check` runs in the project's running instance (RB-092-05).
    if let Some(Command::Check(args)) = &cli.command {
        return Err(super::check::run(
            cli,
            args,
            &env,
            rb_config::local_dir().as_deref(),
        ));
    }
    if matches!(cli.command, Some(Command::Diagnostics(_))) {
        return Ok(None);
    }
    // `rbrowser engines firefox --list` reads this machine, no window (RB-080-32).
    if let Some(Command::Engines(args)) = &cli.command {
        let super::engines::EnginesCommand::Firefox(firefox) = &args.command;
        return Err(super::engines::run(firefox, user));
    }
    // `rbrowser cdp-tunnel` runs the WSL tunnel in the foreground, no window (RB-030-06).
    if let Some(Command::CdpTunnel(args)) = &cli.command {
        return Err(crate::cdp_proxy::tunnel::cli(
            args.port,
            args.distro.as_deref(),
            user,
        ));
    }
    // `rbrowser flow` lists the flows or runs one in the running instance (RB-040-07).
    if let Some(Command::Flow(args)) = &cli.command {
        return Err(crate::flows::cli::main(cli, args, &env, user));
    }
    // `rbrowser mcp` forwards to the running instance and never opens a window (RB-030-01).
    if let Some(Command::Mcp(args)) = &cli.command {
        return Err(crate::mcp::cli::main(cli, args, &env, user));
    }
    let plan = match plan(cli, &env) {
        Ok(plan) => plan,
        Err((code, message)) => {
            tracing::error!("{message}");
            eprintln!("rbrowser: {message}");
            return Err(code);
        }
    };
    if let [first, _, ..] = plan.urls.as_slice() {
        eprintln!("rbrowser: every view shows one page; only {first} is loaded");
    }
    match forward(&plan, rb_config::local_dir().as_deref()) {
        Some(code) => Err(code),
        None => Ok(Some(plan)),
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
