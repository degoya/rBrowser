//! Single instance per project (RB-010-11): the instance file
//! (`<local dir>\instances\<hash>.json`, `rb_config::local_dir`, [`file`]), the local channel
//! open to the same user only ([`channel`]) with its nonce that takes a second call's URL and
//! flags ([`protocol`], [`server`], [`client`]), `rbrowser screenshot` in the instance
//! ([`screenshot`], RB-050-03), and `project.open_recent` with the recent projects ([`recent`]).
//! The CLI side is `crate::cli`.

pub mod channel;
pub mod client;
pub mod file;
pub mod protocol;
pub mod recent;
pub mod screenshot;
pub mod server;

use std::sync::{Arc, Mutex};

use rb_commands::CommandError;
use serde_json::{Value, json};

use crate::control::Core;
use protocol::Request;

/// This process's listener, kept for [`stop_listening`].
static LISTENING: Mutex<Option<server::Listening>> = Mutex::new(None);

/// Runs the command `id` when it is this module's (`project.open_recent`); `None` otherwise.
pub fn run_command(core: &Core, id: &str, args: &Value) -> Option<Result<Value, CommandError>> {
    (id == recent::OPEN_RECENT).then(|| recent::open_recent(core, args))
}

/// The project is open: it goes to the front of the recent projects, and the instance channel
/// starts listening, so a second call for this project reaches this window.
pub fn on_start(core: &Core) {
    let Some(root) = core.host().project_root() else {
        return;
    };
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = crate::cli::paths::absolute(&root, &cwd);
    recent::record(core, &root);
    let Some(local) = rb_config::local_dir() else {
        tracing::warn!("no local folder: a second call for this project starts its own window");
        return;
    };
    let Some(core) = core.arc() else {
        return;
    };
    let weak = Arc::downgrade(&core);
    let handler: server::Handler = Arc::new(move |request| {
        let core = weak
            .upgrade()
            .ok_or_else(|| "the window is closed".to_owned())?;
        handle(&core, request)
    });
    match server::start(&local, &root, handler) {
        Ok(listening) => {
            *rb_util::lock(&LISTENING) = Some(listening);
            tracing::info!(project = %root.display(), "listening for calls for this project");
        }
        Err(error) => tracing::warn!(
            %error,
            "the instance channel did not start; a second call for this project starts its own window"
        ),
    }
}

/// Stops this process's listener ([`server::Listening::stop`]): the quit of `app.restart` runs
/// it before the next start (RB-094-23). Nothing when none listens.
pub fn stop_listening() {
    let listening = rb_util::lock(&LISTENING).take();
    if let Some(listening) = listening {
        listening.stop();
    }
}

/// Carries out a checked request through the same commands the toolbar and MCP use; a
/// screenshot through the same capture as the MCP tool, answering its result.
fn handle(core: &Arc<Core>, request: Request) -> Result<Option<Value>, String> {
    match request {
        Request::Open { urls, background } => {
            if let Some(url) = urls.first() {
                core.run_command("view.navigate", json!({ "url": url }))
                    .map_err(|error| error.to_string())?;
            }
            if !background && core.user_settings().startup.focus_on_open {
                core.ui().activate_window();
            }
            Ok(None)
        }
        Request::Reload { ignore_cache } => core
            .run_command("reload.all", json!({ "ignoreCache": ignore_cache }))
            .map(|_| None)
            .map_err(|error| error.to_string()),
        Request::Screenshot { args, views, out } => screenshot::run(
            &screenshot::CoreCapturer(core.clone()),
            &args,
            &views,
            out.as_deref(),
        )
        .map(Some),
        // The run goes on in the background; the CLI waits for its result.json.
        Request::FlowRun {
            name,
            views,
            run_id,
        } => {
            let views = (!views.is_empty()).then_some(views);
            core.run_command(
                crate::flows::RUN,
                json!({ "name": name, "views": views, "runId": run_id }),
            )
            .map(|_| None)
            .map_err(|error| error.to_string())
        }
        // RB-050-05: answered once the pages are found; the CLI follows run.json.
        Request::SitemapCapture { args } => {
            crate::sitemap::app::capture_blocking(core, args).map(Some)
        }
        // RB-092-05: answered once the run is through and exported.
        Request::Check { args } => crate::checks::export::check::run(core, &args).map(Some),
    }
}
