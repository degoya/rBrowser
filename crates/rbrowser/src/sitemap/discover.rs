//! The two-step start of a sitemap capture in the window (RB-099-09): `sitemap.capture` without
//! arguments opens the start page in the pages window ([`open_start`], the route
//! [`START_ROUTE`]), which finds the pages first (`sitemapStart` `discover`, [`dispatch`]) and
//! starts the run of the pages chosen with `sitemap.capture` `{ discovery, urls, env, views }`.
//! The MCP tool `sitemap_discover` ([`host_tool`]) and `rbrowser capture sitemap --list`
//! ([`discover_blocking`]) find the same way; a capture with `urls` takes only pages of the
//! discovery it names, or of a discovery of its own source ([`selected_run`]).
//!
//! Finding runs the plugin's `discover` as a capture would - the source, `robots.txt`, the
//! patterns of the settings - but every page found is answered (no `sitemap.maxPages`, at most
//! `super::tool::MAX_PAGES`): the person chooses. The environment moves the pages at the
//! capture, not here.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use rb_ipc::{CoreToShell, IpcErrorCode, ShellToCore, SitemapDiscovery, SitemapStartOp};
use rb_util::url::origin_of;
use serde_json::{Value, json};

use super::app::BundledTools;
use super::discover_tool::{DEFAULT_DEPTH, DiscoverArgs};
use super::discovery::{
    Discovered, Discoveries, Found, MAX_URLS_FILE_BYTES, chosen, parse_url_list, run_request,
    view_counts, write_discovery,
};
use super::plan::{self, Breakpoints, PlanError};
use super::runs::{NewRun, PluginTools, RunError, Start, pages_of};
use super::settings::Effective;
use super::tool::CaptureArgs;
use super::tree;
use crate::control::Core;
use crate::ipc::Failure;

/// The start page's route in the shell's pages.
pub const START_ROUTE: &str = "sitemap-start";

/// The discoveries of this process.
pub fn discoveries() -> &'static Discoveries {
    static KEPT: OnceLock<Discoveries> = OnceLock::new();
    KEPT.get_or_init(Discoveries::default)
}

/// What the plugin's `discover` `answer` found for `source`; the rewrite moves the pages from the
/// site's origin, else the source's, else the start URL's.
#[must_use]
pub fn found_of(answer: &Value, source: plan::Source, start_url: Option<&str>) -> Found {
    let site = answer
        .get("site")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let rewrite_from = site
        .clone()
        .or(source.from)
        .or_else(|| start_url.and_then(origin_of));
    Found {
        source: source.label,
        site,
        rewrite_from,
        urls: pages_of(answer).into_iter().map(|(url, _)| url).collect(),
        truncated: answer
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// Finds the pages of `args` in the open project and keeps them under a new discovery id.
///
/// # Errors
///
/// No project, a source that cannot be read, the plugin failed, or no page was found.
pub async fn discover(core: &Arc<Core>, args: &DiscoverArgs) -> Result<Arc<Discovered>, RunError> {
    let host = core.host().clone();
    let root = host.project_root().ok_or(RunError::NoProject)?;
    let root_of_project = root.clone();
    let config = host.config();
    let probe = args.source_args();
    let effective = Effective::new(&config.sitemap, &probe);
    let (project, max_bytes) = (config.clone(), effective.max_bytes);
    let source =
        tokio::task::spawn_blocking(move || plan::source(&probe, &project, &root, max_bytes))
            .await
            .map_err(|error| RunError::Failed(error.to_string()))??;
    let (limits, selection) = effective.discover_args();
    let request = json!({ "source": source.source, "limits": limits, "selection": selection });
    let answer = tokio::task::spawn_blocking(move || BundledTools.discover(&request))
        .await
        .map_err(|error| RunError::Failed(error.to_string()))?
        .map_err(RunError::Discover)?;
    let found = found_of(&answer, source, config.start_url.as_deref());
    if found.urls.is_empty() {
        return Err(RunError::NoPages);
    }
    let id = discoveries().next_id(super::runs::now_unix());
    let made = found.into_discovered(id, view_counts(&config));
    Ok(discoveries().keep(&root_of_project, made))
}

/// The pages a call names: `urls`, or the lines of its project-relative `urlsFile` (read here:
/// the instance channel carries no long list).
///
/// # Errors
///
/// The file is not a readable `.txt` inside the project, too large, or its list is empty or too
/// long.
pub async fn named_urls(core: &Core, args: &CaptureArgs) -> Result<Vec<String>, RunError> {
    if let Some(urls) = &args.urls {
        return Ok(urls.clone());
    }
    let Some(file) = args.urls_file.clone() else {
        return Err(RunError::NoPages);
    };
    let root = core.host().project_root().ok_or(RunError::NoProject)?;
    let max = usize::try_from(MAX_URLS_FILE_BYTES).unwrap_or(usize::MAX);
    tokio::task::spawn_blocking(move || {
        let bad = |reason: String| PlanError::BadFile(format!("{file}: {reason}"));
        let bytes = plan::read_project_file(&root, &file, max)?;
        let text = String::from_utf8(bytes).map_err(|_| bad("not UTF-8 text".to_owned()))?;
        parse_url_list(&text).map_err(bad)
    })
    .await
    .map_err(|error| RunError::Failed(error.to_string()))?
    .map_err(RunError::from)
}

/// The run of the pages the call names ([`named_urls`]) of `args.discovery` - or, without one,
/// of a discovery of the call's own source - in `breakpoints`, moved to `args.env`'s origin.
///
/// # Errors
///
/// The discovery is no longer kept, a URL it did not find, too many URLs, an unknown
/// environment, the URL file, or the discovery's own failure.
pub async fn selected_run(
    core: &Arc<Core>,
    args: &CaptureArgs,
    breakpoints: Breakpoints,
) -> Result<Start, RunError> {
    let urls = named_urls(core, args).await?;
    let urls = urls.as_slice();
    let root = core.host().project_root().ok_or(RunError::NoProject)?;
    let found = match &args.discovery {
        // Only a discovery of the project open now (security review, wave 099-w36).
        Some(id) => discoveries()
            .get(id, &root)
            .ok_or_else(|| RunError::UnknownDiscovery(id.clone()))?,
        None => discover(core, &DiscoverArgs::of_capture(args)).await?,
    };
    let pages = chosen(&found, urls)?;
    let config = core.host().config();
    let env = CaptureArgs {
        env: args.env.clone(),
        ..CaptureArgs::default()
    };
    let from = plan::Source {
        source: Value::Null,
        label: found.answer.source.clone(),
        from: found.rewrite_from.clone(),
    };
    let rewrite = plan::rewrite(&env, &config, &from)?;
    Ok(Start::New(NewRun {
        discover: run_request(pages, rewrite),
        source: found.answer.source.clone(),
        env: args.env.clone(),
        breakpoints,
    }))
}

/// `found`'s answer with its tree cut to `depth` levels.
#[must_use]
pub fn answer_of(found: &Discovered, depth: usize) -> SitemapDiscovery {
    let mut answer = found.answer.clone();
    tree::prune(&mut answer.tree, depth);
    answer
}

/// Counts the start pages opened: each gets an address of its own, so the page finds the pages
/// again when it is opened once more.
static OPENED: AtomicU64 = AtomicU64::new(0);

/// Opens the start page in the pages window (`sitemap.capture` without arguments).
pub fn open_start(core: &Core) {
    let opened = OPENED.fetch_add(1, Ordering::Relaxed);
    core.ui()
        .open_settings_window(crate::shell::page_url(&format!(
            "{START_ROUTE}?open={opened}"
        )));
}

/// Whether `message` is the start page's: [`dispatch`] answers it.
#[must_use]
pub fn is_request(message: &ShellToCore) -> bool {
    matches!(message, ShellToCore::SitemapStart(_))
}

fn failure(error: &RunError) -> Failure {
    let code = match error {
        RunError::NoProject | RunError::NoPages => IpcErrorCode::NotFound,
        RunError::Plan(_) => IpcErrorCode::BadRequest,
        _ => IpcErrorCode::Internal,
    };
    // The message starts with the code (an i18n key); a URL's credentials are masked.
    (code, rb_diag::redact::redact_text(&error.to_string()))
}

/// Answers `sitemapStart`: `close` at once, `discover` on the core's runtime (the plugin reads
/// the site's sitemaps; the CEF UI thread never waits for them).
pub fn dispatch(
    core: &Core,
    message: ShellToCore,
    reply: impl FnOnce(Result<CoreToShell, Failure>) + Send + 'static,
) {
    let ShellToCore::SitemapStart(request) = message else {
        reply(Err((
            IpcErrorCode::BadRequest,
            "no sitemap start request".to_owned(),
        )));
        return;
    };
    if request.op == SitemapStartOp::Close {
        core.ui().close_settings_window();
        reply(Ok(CoreToShell::Ack));
        return;
    }
    let Some(core) = core.arc() else {
        reply(Err((
            IpcErrorCode::Internal,
            "the window is closed".to_owned(),
        )));
        return;
    };
    let runtime = core.runtime().clone();
    runtime.spawn(async move {
        let found = discover(&core, &DiscoverArgs::default()).await;
        reply(
            found
                .map(|found| CoreToShell::SitemapDiscovery(found.answer.clone()))
                .map_err(|error| failure(&error)),
        );
    });
}

/// The MCP tool `sitemap_discover`, its discovery on the core's runtime.
///
/// # Errors
///
/// Arguments that do not read, or the discovery's refusal.
pub async fn host_tool(core: &Arc<Core>, args: Value) -> Result<Value, rb_mcp::ToolFailure> {
    let args = DiscoverArgs::parse(&args)
        .map_err(|message| rb_mcp::ToolFailure::new("args.invalid", message))?;
    let depth = args.depth.unwrap_or(DEFAULT_DEPTH);
    let run = core.clone();
    let found = core
        .runtime()
        .spawn(async move { discover(&run, &args).await })
        .await
        .map_err(|error| rb_mcp::ToolFailure::new("mcp.internal", error.to_string()))?
        .map_err(|error| super::app::tool_failure(&error))?;
    serde_json::to_value(answer_of(&found, depth))
        .map_err(|error| rb_mcp::ToolFailure::new("mcp.internal", error.to_string()))
}

/// [`discover`] for the instance channel (`rbrowser capture sitemap --list`), from a thread that
/// is no async task: waited for at most `super::app::DISCOVER_BUDGET`. The discovery is written
/// to `.rbrowser/out/sitemap-discover/<discoveryId>.json`; the answer names the file.
///
/// # Errors
///
/// The discovery's refusal (its code first), or no answer in time.
pub fn discover_blocking(core: &Arc<Core>, args: DiscoverArgs) -> Result<Value, String> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let depth = args.depth.unwrap_or(DEFAULT_DEPTH);
    let run = core.clone();
    core.runtime().spawn(async move {
        let found = discover(&run, &args).await;
        let _ = sender.send(found);
    });
    let found = receiver
        .recv_timeout(super::app::DISCOVER_BUDGET)
        .map_err(|_| "the sitemap was not read in time".to_owned())?
        .map_err(|error| rb_diag::redact::redact_text(&error.to_string()))?;
    let root = core
        .host()
        .project_root()
        .ok_or_else(|| RunError::NoProject.to_string())?;
    // The answer goes to a file: thousands of pages are more than the channel's reply.
    let written = write_discovery(&root, &answer_of(&found, depth))?;
    serde_json::to_value(written).map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "discover_tests.rs"]
mod tests;
