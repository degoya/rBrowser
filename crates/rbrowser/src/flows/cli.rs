//! `rbrowser flow list` and `rbrowser flow run <name> [--views a,b] [--json]`. Listing reads the
//! project's files and needs no window. A run happens in the project's running rBrowser (its
//! views): the CLI checks trust and the flow first, hands the run over the instance channel with
//! a run id of its own, and waits for that run's `result.json` - the same document the shell
//! and `flow_run` show. Exit code 1 when a view failed or timed out, or the run was refused.

use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use clap::{Args, Subcommand};
use rb_config::UserSettings;

use super::result::{RunResult, ViewStatus, read, run_dir, run_id};
use super::{FlowError, catalog, gate, run, scope};
use crate::cli::plan::{Env, plan};
use crate::cli::{Cli, exit};
use crate::instance::client::{Outcome, hand_over};
use crate::instance::protocol::Request;

/// How often the CLI looks for the result.
const POLL: Duration = Duration::from_millis(200);

/// `rbrowser flow`.
#[derive(Debug, Args)]
pub struct FlowArgs {
    /// What to do.
    #[command(subcommand)]
    pub command: FlowCommand,
}

/// The subcommands of `rbrowser flow`.
#[derive(Debug, Subcommand)]
pub enum FlowCommand {
    /// List the flows of the project (`.rbrowser/flows/*.ts`) and whether they may run.
    List {
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run a flow in the views of the project's running rBrowser and wait for the result; exit
    /// code 1 when a view failed. Needs a trusted workspace.
    Run {
        /// The flow's name (its file name without `.ts`).
        name: String,
        /// The views: ids, breakpoint ids or labels, comma-separated; the flow's `// @views` or
        /// all when absent.
        #[arg(long, value_delimiter = ',')]
        views: Vec<String>,
        /// Print the result as JSON (`result.json`).
        #[arg(long)]
        json: bool,
    },
}

/// Runs `rbrowser flow` in `env`; returns the exit code.
pub fn main(cli: &Cli, args: &FlowArgs, env: &Env, user: &UserSettings) -> i32 {
    let root = match plan(cli, env) {
        Ok(plan) => plan.root,
        Err((code, message)) => {
            eprintln!("rbrowser flow: {message}");
            return code;
        }
    };
    let Some(root) = root else {
        eprintln!("rbrowser flow: no project: pass --project DIR or run it inside the project");
        return exit::NO_PROJECT;
    };
    let settings = &user.automation.flows;
    let store = rb_config::trust::trust_store_path();
    match &args.command {
        FlowCommand::List { json } => list(&root, &settings.dir, store.as_deref(), *json),
        FlowCommand::Run { name, views, json } => {
            let checked = gate::trusted_hash(&root, store.as_deref())
                .and_then(|_| catalog::read(&root, &settings.dir, name));
            let source = match checked {
                Ok((_, source)) => source,
                Err(error) => {
                    eprintln!("rbrowser flow run: {error}");
                    return exit::FAILED;
                }
            };
            let timeout = catalog::parse_head(&source)
                .timeout_ms
                .unwrap_or(settings.timeout_ms);
            let wait = run::budget(scope::SLOTS, settings.parallel, timeout) + run::RUN_GRACE;
            let id = new_run_id();
            let request = Request::FlowRun {
                name: name.clone(),
                views: views.clone(),
                run_id: id.clone(),
            };
            let Some(local) = rb_config::local_dir() else {
                eprintln!("{}", no_local_folder(cfg!(windows)));
                return exit::FAILED;
            };
            if let Some(code) = handed(hand_over(&local, &root, request), &root) {
                return code;
            }
            match wait_for(&root, &id, wait) {
                Some(result) => {
                    print_result(&result, *json);
                    exit_code(&result)
                }
                None => {
                    eprintln!(
                        "rbrowser flow run: no result after {} s; see the notices in rBrowser",
                        wait.as_secs()
                    );
                    exit::FAILED
                }
            }
        }
    }
}

/// The message when [`rb_config::local_dir`] finds no folder: the variables it comes from on
/// Windows, else on macOS and Linux (RB-097-03).
fn no_local_folder(windows: bool) -> &'static str {
    if windows {
        "rbrowser flow run: no local folder (LOCALAPPDATA is not set)"
    } else {
        "rbrowser flow run: no local folder (neither XDG_STATE_HOME nor HOME is set)"
    }
}

fn list(root: &Path, dir: &str, store: Option<&Path>, json: bool) -> i32 {
    let flows = match catalog::list(root, dir) {
        Ok(flows) => flows,
        Err(error) => {
            eprintln!("rbrowser flow list: {error}");
            return exit::FAILED;
        }
    };
    let trusted = gate::trusted_hash(root, store).is_ok();
    if json {
        let answer = serde_json::json!({ "dir": dir, "trusted": trusted, "flows": flows });
        println!("{answer:#}");
        return exit::OK;
    }
    for flow in &flows {
        match &flow.description {
            Some(description) => println!("{}  {description}", flow.name),
            None => println!("{}", flow.name),
        }
    }
    if flows.is_empty() {
        println!("no flows in {dir}");
    }
    if !trusted {
        println!("{}", FlowError::Untrusted);
    }
    exit::OK
}

/// `Some(exit code)` when the hand-over did not happen.
fn handed(outcome: Outcome, root: &Path) -> Option<i32> {
    match outcome {
        Outcome::Handed | Outcome::Answered(_) => None,
        Outcome::NotRunning => {
            eprintln!(
                "rbrowser flow run: no rBrowser runs for {}; start it with: rbrowser --project \"{}\"",
                root.display(),
                root.display()
            );
            Some(exit::NO_INSTANCE)
        }
        Outcome::Refused(refusal, message) => {
            let why = message.unwrap_or_else(|| "its notice says why".to_owned());
            eprintln!(
                "rbrowser flow run: the running rBrowser refused the run ({refusal:?}): {why}"
            );
            Some(exit::HANDOVER_FAILED)
        }
        Outcome::Failed(error) => {
            eprintln!("rbrowser flow run: the running rBrowser did not answer: {error}");
            Some(exit::HANDOVER_FAILED)
        }
    }
}

fn new_run_id() -> String {
    let started = crate::trust::rfc3339(SystemTime::now());
    let random = crate::secret::random_hex(3).unwrap_or_else(|_| "000000".to_owned());
    run_id(&started, &random)
}

/// The result of the run `id` of `root` once it is written; `None` after `wait`.
pub fn wait_for(root: &Path, id: &str, wait: Duration) -> Option<RunResult> {
    let dir = run_dir(root, id);
    let deadline = Instant::now() + wait;
    loop {
        if let Some(result) = read(&dir) {
            return Some(result);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(POLL);
    }
}

/// The exit code of a finished run.
pub fn exit_code(result: &RunResult) -> i32 {
    if result.passed {
        exit::OK
    } else {
        exit::FAILED
    }
}

fn print_result(result: &RunResult, json: bool) {
    if json {
        match serde_json::to_string_pretty(result) {
            Ok(text) => println!("{text}"),
            Err(error) => eprintln!("rbrowser flow run: {error}"),
        }
    } else {
        print!("{}", text(result));
    }
}

/// The result for a person: a summary line, then one line per view (and its error).
pub fn text(result: &RunResult) -> String {
    let (passed, failed, skipped) = result.counts();
    let mut out = format!(
        "{}: {passed} passed, {failed} failed, {skipped} skipped ({} ms) - {}/{}\n",
        result.flow,
        result.duration_ms,
        super::result::OUT_DIR,
        result.run_id
    );
    if let Some(error) = &result.error {
        out.push_str(&format!("  did not run: {error}\n"));
    }
    for view in &result.views {
        let status = match view.status {
            ViewStatus::Passed => "passed",
            ViewStatus::Failed => "FAILED",
            ViewStatus::Skipped => "skipped",
            ViewStatus::TimedOut => "TIMED OUT",
        };
        out.push_str(&format!(
            "  {status:<9} {} [{}] {} ms\n",
            view.label, view.engine, view.duration_ms
        ));
        if let Some(reason) = &view.reason {
            out.push_str(&format!("            {reason}\n"));
        }
        if let Some(error) = &view.error {
            let line = error
                .line
                .map_or(String::new(), |line| format!(" ({}:{line})", error.file));
            let step = error
                .step
                .as_ref()
                .map_or(String::new(), |step| format!(" in step {step:?}"));
            let first = error.message.lines().next().unwrap_or_default();
            out.push_str(&format!("            {first}{step}{line}\n"));
        }
        if let Some(screenshot) = &view.screenshot {
            out.push_str(&format!(
                "            screenshot: {}/{}/{screenshot}\n",
                super::result::OUT_DIR,
                result.run_id
            ));
        }
    }
    out
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
