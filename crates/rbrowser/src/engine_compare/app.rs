//! The engine comparison in the window: the command, the MCP host tool's entry point and the
//! notices. The target view (the active one by default) gives the breakpoint and the page;
//! Chromium's shot is taken in a hidden render view of the sitemap capture
//! (`crate::sitemap::render_views`), Firefox's and WebKit's in a window of the engine's own
//! ([`super::backend`]); an engine that cannot run is left out and named. Everything runs on the
//! core's runtime, the diff and the files on blocking threads; the grid is not touched.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use rb_commands::CommandError;
use rb_config::EngineKind;
use rb_engine::chromium::hidden::RenderBackend;
use rb_engine::{Engine, EngineHealth, Target, ViewHost as _, engine_name};
use rb_ipc::{CoreToShell, Notification, NotificationLevel};
use serde_json::{Value, json};

use super::backend::EngineBackend;
use super::plan::{self, PlanError};
use super::report::EngineSummary;
use super::run::{EngineRunError, EngineRuns, NewEngineRun};
use super::tool::{COMPARE_TOOL, CompareEnginesArgs};
use crate::control::Core;

/// The notice of a started comparison; placeholder `{engines}`.
pub const STARTED_KEY: &str = "enginecompare.notice.started";
/// The notice of a finished comparison; placeholders `{differing}`, `{pairs}`, `{folder}`.
pub const FINISHED_KEY: &str = "enginecompare.notice.finished";
/// The notice of the engines left out; placeholder `{engines}`.
pub const SKIPPED_KEY: &str = "enginecompare.notice.skipped";
/// The notice of a comparison that failed; placeholder `{error}`.
pub const FAILED_KEY: &str = "enginecompare.notice.failed";
/// The reason of an external engine this window has not started; placeholder `{engine}`.
pub const NOT_STARTED_KEY: &str = "enginecompare.not_started";
/// The settings section the notices link to.
const SETTINGS_KEY: &str = "engine-compare";

/// The comparisons of this process (one window).
pub fn runs() -> &'static Arc<EngineRuns> {
    static RUNS: OnceLock<Arc<EngineRuns>> = OnceLock::new();
    RUNS.get_or_init(EngineRuns::new)
}

fn notice(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect::<BTreeMap<_, _>>(),
        settings_key: Some(SETTINGS_KEY.to_owned()),
    })
}

/// The external engine of `kind` in `core`'s window; `None` for Chromium or one not started.
fn external_engine(core: &Core, kind: EngineKind) -> Option<Arc<dyn Engine>> {
    match kind {
        EngineKind::Chromium => None,
        EngineKind::Firefox => {
            crate::engines::firefox::for_host(core.host()).map(|bridge| bridge.engine())
        }
        EngineKind::Webkit => {
            crate::engines::webkit::for_host(core.host()).map(|engine| engine as Arc<dyn Engine>)
        }
        EngineKind::Android => {
            crate::engines::android::for_host(core.host()).map(|engine| engine as Arc<dyn Engine>)
        }
        EngineKind::Safari => {
            crate::engines::safari::for_host(core.host()).map(|engine| engine as Arc<dyn Engine>)
        }
        EngineKind::Ios => {
            crate::engines::ios::for_host(core.host()).map(|engine| engine as Arc<dyn Engine>)
        }
    }
}

/// How the engine of `kind` is doing for a comparison: Chromium always runs.
fn health(core: &Core, kind: EngineKind) -> EngineHealth {
    if kind == EngineKind::Chromium {
        return EngineHealth::Running;
    }
    external_engine(core, kind).map_or_else(
        || {
            EngineHealth::Unavailable(rb_i18n::t_with(
                NOT_STARTED_KEY,
                &[("engine", engine_name(kind))],
            ))
        },
        |engine| engine.health(),
    )
}

/// Who asked for a comparison: the person's command may compare any page, an agent (MCP) only
/// what MCP's `navigate` may load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caller {
    /// The command `compare.engines`.
    Person,
    /// The MCP tool `compare_engines`.
    Agent,
}

/// Compares the engines and answers when done: the MCP tool `compare_engines` and the command
/// `compare.engines`.
///
/// # Errors
///
/// No project, a call that cannot be planned, or another comparison in progress.
pub async fn compare(
    core: &Arc<Core>,
    args: CompareEnginesArgs,
    caller: Caller,
) -> Result<EngineSummary, EngineRunError> {
    let host = core.host().clone();
    let root = host.project_root().ok_or(EngineRunError::NoProject)?;
    let config = host.config();
    let views = host.views();
    let active = host.active_view();
    let target = args
        .view
        .and_then(|id| views.iter().find(|view| view.id.0 == id))
        .or_else(|| views.iter().find(|view| Some(view.id) == active))
        .or_else(|| views.first());
    let shown = target.and_then(|view| {
        view.cell_breakpoint
            .clone()
            .or_else(|| view.breakpoint.clone())
    });
    let id = plan::breakpoint_id(args.breakpoint.as_deref(), shown.as_deref())?;
    let breakpoint = config
        .breakpoints
        .iter()
        .find(|breakpoint| breakpoint.id == id)
        .cloned()
        .ok_or(PlanError::UnknownBreakpoint(id))?;
    let url = plan::page_url(args.url.as_deref(), target.map(|view| view.url.as_str()))?;
    let url = match caller {
        Caller::Person => url,
        Caller::Agent => plan::agent_url(&url, Some(root.as_path()))?,
    };
    let trusted = host.shell_state().trusted;
    let (shot, spec) = plan::shot_spec(
        &breakpoint,
        &config.emulation,
        &crate::emulation::device_list(),
        trusted,
    )?;
    let (engines, skipped) =
        plan::partition(&plan::engines(&args.engines)?, |kind| health(core, kind));
    let mut backends: Vec<Arc<dyn RenderBackend>> = Vec::new();
    for &kind in &engines {
        // No capture preparation (banners, animations): Firefox and WebKit have none, and the
        // three shots are taken under the same conditions.
        let backend: Arc<dyn RenderBackend> = match (kind, external_engine(core, kind)) {
            (EngineKind::Chromium, _) => Arc::new(crate::sitemap::render_views::backend_for(&host)),
            (_, Some(engine)) => Arc::new(EngineBackend::new(
                engine,
                breakpoint.clone(),
                crate::engines::window_title(&shot.label, kind),
            )),
            (_, None) => {
                return Err(EngineRunError::Failed(format!(
                    "{} stopped before the comparison",
                    engine_name(kind)
                )));
            }
        };
        backends.push(backend);
    }
    if !skipped.is_empty() {
        let names: Vec<&str> = skipped.iter().map(|s| s.engine.as_str()).collect();
        core.notify(notice(
            NotificationLevel::Warning,
            SKIPPED_KEY,
            &[("engines", names.join(", "))],
        ));
    }
    let names: Vec<&str> = engines.iter().map(|kind| engine_name(*kind)).collect();
    core.notify(notice(
        NotificationLevel::Info,
        STARTED_KEY,
        &[("engines", names.join(", "))],
    ));
    let effective = plan::effective(
        &config.engine_compare,
        config.visual.threshold,
        &args.overrides(),
    );
    let new = NewEngineRun {
        root,
        now_unix: crate::sitemap::runs::now_unix(),
        run: super::report::EngineRun::new(&url, shot, &engines, skipped, effective.max_ratio),
        spec,
        backends,
        batch: effective.batch,
        options: effective.options,
        texts: Arc::new(|key: &str| rb_i18n::t(&format!("enginecompare.report.{key}"))),
    };
    let summary = runs().run(new).await?;
    let level = if summary.differing == 0 {
        NotificationLevel::Info
    } else {
        NotificationLevel::Warning
    };
    core.notify(notice(
        level,
        FINISHED_KEY,
        &[
            ("differing", summary.differing.to_string()),
            ("pairs", summary.pairs.len().to_string()),
            ("folder", summary.folder.clone()),
        ],
    ));
    Ok(summary)
}

/// Opens `summary`'s report in the active view (the first one without).
fn open_report(core: &Core, summary: &EngineSummary) -> Result<(), String> {
    let report = summary
        .report
        .as_deref()
        .ok_or_else(|| "engine_compare.error.no_report".to_owned())?;
    let host = core.host();
    let view = host
        .active_view()
        .or_else(|| host.views().first().map(|view| view.id))
        .ok_or_else(|| "there is no view".to_owned())?;
    host.navigate(Target::View(view), &crate::compare::app::file_url(report))
        .map_err(|error| error.to_string())
}

/// Runs `compare.engines`: answers at once and goes on on the core's runtime (a command may
/// come from the CEF UI thread); a comparison that fails is a notice.
pub fn run(core: &Core, args: &Value) -> Result<Value, CommandError> {
    let args = CompareEnginesArgs::parse(args).map_err(CommandError::InvalidArgs)?;
    let core = core
        .arc()
        .ok_or_else(|| CommandError::Failed("the window is closed".to_owned()))?;
    let runtime = core.runtime().clone();
    runtime.spawn(async move {
        let open = args.open.unwrap_or(true);
        let outcome = compare(&core, args, Caller::Person)
            .await
            .map_err(|error| (error.code(), error.to_string()))
            .and_then(|summary| {
                if open {
                    open_report(&core, &summary)
                        .map_err(|error| ("engine_compare.error.report", error))
                } else {
                    Ok(())
                }
            });
        if let Err((code, error)) = outcome {
            tracing::warn!(code, "an engine comparison did not complete");
            let args = [("error", rb_diag::redact::redact_text(&error))];
            core.notify(notice(NotificationLevel::Error, FAILED_KEY, &args));
        }
    });
    Ok(json!({ "started": true }))
}

/// The MCP tools the host runs here.
pub const TOOLS: [&str; 1] = [COMPARE_TOOL];

/// Runs the MCP tool `name` (one of [`TOOLS`]) with `args`, on the core's runtime, to the end.
///
/// # Errors
///
/// Arguments that do not read, the comparison's refusal, or an unknown name.
pub async fn host_tool(
    core: &Arc<Core>,
    name: &str,
    args: Value,
) -> Result<Value, rb_mcp::ToolFailure> {
    if name != COMPARE_TOOL {
        return Err(rb_mcp::ToolFailure::new(
            "unknown_tool",
            format!("the host runs no tool `{name}`"),
        ));
    }
    let mut args = CompareEnginesArgs::parse(&args)
        .map_err(|message| rb_mcp::ToolFailure::new("invalid_arguments", message))?;
    args.open = Some(false);
    let run = core.clone();
    let summary = core
        .runtime()
        .spawn(async move { compare(&run, args, Caller::Agent).await })
        .await
        .map_err(|error| rb_mcp::ToolFailure::new("internal", error.to_string()))?
        .map_err(|error| {
            rb_mcp::ToolFailure::new(
                error.code(),
                rb_diag::redact::redact_text(&error.to_string()),
            )
        })?;
    serde_json::to_value(&summary)
        .map_err(|error| rb_mcp::ToolFailure::new("internal", error.to_string()))
}
