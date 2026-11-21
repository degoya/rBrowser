//! The host side of the breakpoint analyser (RB-010-13): the hidden Chromium target whose CSS
//! the plugin `breakpoint-analyzer` reads ([`cdp_source`]), the proposal for the wizard
//! (`analyzeBreakpoints` -> `breakpointProposal`), writing it (`applyBreakpoints`), the found
//! breakpoints in [`crate::host::shell_state::ShellState::detected`], `rbrowser init`'s
//! proposal ([`init_config`]), the loader of the bundled plugins ([`plugins`]) and the project
//! files of the static source ([`project_files`], RB-030-07).
//!
//! - `breakpoints.analyze` (`Leader B A`): without arguments (shortcut, toolbar, palette) it
//!   opens the wizard (`page_url("analyzer")`); with arguments (MCP `breakpoints_detect`:
//!   `{ urls?, mode? }`) it answers the proposal and writes nothing.
//! - `breakpoints.append_detected` (`Leader B N`, MCP `breakpoints_apply`: `{ selected?,
//!   replace? }`): writes the chosen views of the last proposal (analysing first when there is
//!   none) - all new ones without `selected`.
//!
//! Nothing is written without one of those or the wizard's `applyBreakpoints`.

pub mod cdp_source;
pub mod fetch;
pub mod html;
pub mod init;
pub mod plugins;
pub mod project_files;
pub mod run;
pub mod source;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use rb_commands::CommandError;
use rb_config::AnalyzerMode;
use rb_ipc::{
    BreakpointProposal, CoreToShell, IpcErrorCode, Notification, NotificationLevel,
    ProposedBreakpoint, ShellToCore,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::control::Core;
use crate::ipc::Failure;
use run::AnalyzerError;
use source::Collected;

/// The last proposal, which `applyBreakpoints` and `breakpoints.append_detected` write from.
struct Last {
    id: u32,
    root: Option<PathBuf>,
    breakpoints: Vec<ProposedBreakpoint>,
}

static LAST: Mutex<Option<Last>> = Mutex::new(None);
static NEXT_ID: AtomicU32 = AtomicU32::new(1);
static BUSY: AtomicBool = AtomicBool::new(false);

/// Work for the background thread; its failure becomes a notification.
type Work = Box<dyn FnOnce(&Core) -> Result<(), AnalyzerError> + Send>;

/// Holds [`BUSY`] while one analysis runs.
struct Running;

impl Running {
    fn start() -> Result<Self, AnalyzerError> {
        BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| AnalyzerError::Busy)
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

/// The arguments of `breakpoints.analyze` (MCP `breakpoints_detect`).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DetectRequest {
    urls: Vec<String>,
    mode: Option<AnalyzerMode>,
}

/// The arguments of `breakpoints.append_detected` (MCP `breakpoints_apply`).
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ApplyRequest {
    proposal_id: Option<u32>,
    selected: Vec<String>,
    replace: bool,
}

/// Runs the command `id` when it is this module's (`breakpoints.analyze`, `breakpoints.append_detected`); `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    let failed = |error: AnalyzerError| CommandError::Failed(error.to_string());
    let invalid = |error: serde_json::Error| CommandError::InvalidArgs(error.to_string());
    match id {
        "breakpoints.analyze" if args.is_null() => {
            core.ui()
                .open_settings_window(crate::shell::page_url("analyzer"));
            Some(Ok(Value::Null))
        }
        "breakpoints.analyze" => Some((|| {
            let request: DetectRequest = serde_json::from_value(args.clone()).map_err(invalid)?;
            let proposal = analyze(core, &request.urls, request.mode).map_err(failed)?;
            serde_json::to_value(proposal).map_err(|error| CommandError::Failed(error.to_string()))
        })()),
        "breakpoints.append_detected" => Some((|| {
            let request: ApplyRequest = if args.is_null() {
                ApplyRequest::default()
            } else {
                serde_json::from_value(args.clone()).map_err(invalid)?
            };
            let added = append(core, &request).map_err(failed)?;
            Ok(json!({ "added": added }))
        })()),
        _ => None,
    }
}

/// Answers `message` when it is this module's (`analyzeBreakpoints`, `applyBreakpoints`); `None` otherwise.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let request = match message {
        ShellToCore::AnalyzeBreakpoints { urls, mode } => {
            let (urls, mode) = (urls.clone(), *mode);
            Box::new(move |core: &Core| {
                analyze(core, &urls, mode)
                    .map(|proposal| core.send(&CoreToShell::BreakpointProposal(proposal)))
            }) as Work
        }
        ShellToCore::ApplyBreakpoints {
            proposal_id,
            selected,
            replace,
        } => {
            let request = ApplyRequest {
                proposal_id: Some(*proposal_id),
                selected: selected.clone(),
                replace: *replace,
            };
            Box::new(move |core: &Core| append(core, &request).map(|_| ()))
        }
        _ => return None,
    };
    Some(in_background(core, request).map(|()| CoreToShell::Ack))
}

/// Runs `work` off the UI thread; a failure becomes a notification.
fn in_background(core: &Core, work: Work) -> Result<(), Failure> {
    let core = core
        .arc()
        .ok_or_else(|| (IpcErrorCode::Internal, "the window is closed".to_owned()))?;
    let runtime = core.runtime().clone();
    std::thread::Builder::new()
        .name("rb-analyzer".to_owned())
        .spawn(move || {
            let _runtime = runtime.enter();
            if let Err(error) = work(&core) {
                tracing::warn!(%error, "the breakpoint analysis failed");
                core.notify(notification(NotificationLevel::Error, &error.message()));
            }
        })
        .map(|_| ())
        .map_err(|error| (IpcErrorCode::Internal, error.to_string()))
}

fn notification(
    level: NotificationLevel,
    (key, args): &(String, Vec<(String, String)>),
) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.clone(),
        args: args.iter().cloned().collect(),
        settings_key: None,
    })
}

/// `analyzer.checkOnOpen`: look for new breakpoints when the project opens (a notice, nothing
/// written). The bundled plugins start loading in any case.
pub fn on_start(core: &Core) {
    plugins::preload(core.runtime().clone(), core.host().project_root());
    let host = core.host();
    let config = host.config();
    if !config.analyzer.check_on_open || host.project_root().is_none() {
        return;
    }
    let check = Box::new(|core: &Core| {
        let proposal = analyze(core, &[], None)?;
        let new = proposal
            .breakpoints
            .iter()
            .filter(|view| view.is_new)
            .count();
        if new > 0 {
            let message = (
                "analyzer.notice.new_breakpoints".to_owned(),
                vec![("count".to_owned(), new.to_string())],
            );
            let CoreToShell::Notification(mut notice) =
                notification(NotificationLevel::Info, &message)
            else {
                return Ok(());
            };
            notice.settings_key = Some("analyzer.checkOnOpen".to_owned());
            core.notify(CoreToShell::Notification(notice));
        }
        Ok(())
    });
    if let Err((_, error)) = in_background(core, check) {
        tracing::warn!(%error, "the check for new breakpoints did not start");
    }
}

/// Analyses the project's pages (and `urls`) in hidden targets, with the project files of the
/// static source when `analyzer.static` is on: the proposal, kept as the last one, its
/// boundaries in the shell state.
fn analyze(
    core: &Core,
    urls: &[String],
    mode: Option<AnalyzerMode>,
) -> Result<BreakpointProposal, AnalyzerError> {
    let _running = Running::start()?;
    let host = core.host();
    let config = host.config();
    let pages = run::pages(&config, urls, false);
    if pages.is_empty() {
        return Err(AnalyzerError::NoUrl);
    }
    let router = host.cdp().clone();
    let collected = plugins::block_on(async move {
        let mut collected = Collected::default();
        let mut failures = Vec::new();
        for page in &pages {
            match cdp_source::collect(&router, page).await {
                Ok(found) => collected.extend(found),
                Err(error) => failures.push(error),
            }
        }
        if collected.sheets.is_empty() && !failures.is_empty() {
            return Err(AnalyzerError::Collect(failures.join("; ")));
        }
        Ok(collected)
    })
    .map_err(|error| AnalyzerError::Plugin(error.into()))??;
    // One snapshot for the whole analysis (RB-092-06).
    let loaded = plugins::bundled();
    let files = run::static_files(&loaded, &config, host.project_root().as_deref());
    let detection = run::detect(&loaded, &config, &collected, &files, mode)?;
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    *lock(&LAST) = Some(Last {
        id,
        root: host.project_root(),
        breakpoints: detection.breakpoints.clone(),
    });
    let detected = detection.detected.clone();
    host.update_shell_state(move |state| state.detected = detected);
    // The footers name the queries of this analysis (RB-080-44).
    host.footer_analysed();
    Ok(BreakpointProposal {
        proposal_id: id,
        existing: !config.breakpoints.is_empty(),
        detected: detection.detected,
        sources: detection.sources,
        breakpoints: detection.breakpoints,
        hints: detection.hints,
    })
}

/// Writes the chosen views of the last proposal (or of a new analysis) into the project.
fn append(core: &Core, request: &ApplyRequest) -> Result<Vec<String>, AnalyzerError> {
    let root = core.host().project_root().ok_or(AnalyzerError::NoProject)?;
    let current = {
        let last = lock(&LAST);
        last.as_ref()
            .filter(|last| last.root.as_ref() == Some(&root))
            .filter(|last| request.proposal_id.is_none_or(|id| id == last.id))
            .map(|last| last.breakpoints.clone())
    };
    let proposed = match (current, request.proposal_id) {
        (Some(proposed), _) => proposed,
        (None, Some(_)) => return Err(AnalyzerError::NoProposal),
        (None, None) => analyze(core, &[], None)?.breakpoints,
    };
    let added = run::write(
        &plugins::bundled(),
        &root,
        &proposed,
        &request.selected,
        request.replace,
    )?;
    let message = (
        "analyzer.notice.applied".to_owned(),
        vec![("count".to_owned(), added.len().to_string())],
    );
    core.notify(notification(NotificationLevel::Info, &message));
    Ok(added)
}

/// The config `rbrowser init` writes for the site at `url` in the folder `root`, proposed by
/// the analyser with the stylesheets it read; without one, why (RB-091-08), and the CLI writes
/// its minimal mobile + desktop config (RB-010-11). Runs without a window: the page and its
/// stylesheets are fetched (see [`fetch`]), the bundled plugin proposes. While the first-run
/// wizard's plugin choice is open the plugins are not loaded yet: the reason is
/// `plugin_missing`, never an empty load kept for the process.
///
/// # Errors
///
/// [`crate::cli::init::NoProposal`]: why the analyser has no proposal.
pub fn init_config(root: &std::path::Path, url: Option<&str>) -> crate::cli::init::Analysis {
    init::init_proposal_with(root, url, &plugins::bundled(), &fetch::HttpFetch::default())
}
