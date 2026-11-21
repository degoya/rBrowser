//! The panels window's requests (`pluginPanel`, RB-060-12), answered off the UI thread: a
//! plugin's command or tool may take its whole time budget.

use std::collections::BTreeSet;
use std::sync::Arc;

use rb_ipc::{CoreToShell, IpcErrorCode, encode_response};
use serde_json::{Map, Value, json};

use super::panels::{Call, PanelRequest, checked_call, panels_of, with_theme};
use super::runtime::PluginRuntime;
use super::{commands::failure_text, for_host};
use crate::control::Core;
use crate::ipc::Failure;

/// Answers `request` through `reply` on the core's runtime.
pub fn dispatch(
    core: &Core,
    request: Value,
    reply: impl FnOnce(Result<String, Failure>) + Send + 'static,
) {
    let Some(core) = core.arc() else {
        reply(Err((
            IpcErrorCode::Internal,
            "the window is closed".to_owned(),
        )));
        return;
    };
    let runtime = core.runtime().clone();
    runtime.spawn(async move {
        let answer = answer(&core, request).await.and_then(|message| {
            encode_response(&CoreToShell::PluginPanel { message })
                .map_err(|error| (error.code(), error.to_string()))
        });
        reply(answer);
    });
}

async fn answer(core: &Arc<Core>, request: Value) -> Result<Value, Failure> {
    let request: PanelRequest = serde_json::from_value(request)
        .map_err(|error| (IpcErrorCode::BadRequest, error.to_string()))?;
    match request {
        // Answered while the plugins still load, and with none: the list says which.
        PanelRequest::Installed => installed(core).await,
        PanelRequest::List => {
            let runtime = runtime_of(core)?;
            Ok(json!({ "panels": panels_offered(core, &runtime) }))
        }
        PanelRequest::Call {
            plugin,
            command,
            tool,
            args,
        } => {
            let runtime = runtime_of(core)?;
            has_panels(core, &runtime, &plugin)?;
            // RB-090-44: a panel's messages are held to `plugins.panelMessageMaxBytes`.
            let limit = core.user_settings().plugins.panel_message_limit();
            let call = checked_call(command, tool, &args, limit)
                .map_err(|reason| (IpcErrorCode::BadRequest, reason))?;
            let args = if args.is_null() { json!({}) } else { args };
            let (name, value) = match call {
                Call::Command(command) => (
                    command.clone(),
                    runtime
                        .panel_run_command(&plugin, &command, &args, limit)
                        .await,
                ),
                Call::Tool(tool) => (
                    tool.clone(),
                    runtime.panel_call_tool(&plugin, &tool, &args, limit).await,
                ),
            };
            // The code form on purpose (RB-080-58): the panel is the plugin's own page and
            // translates the code with its own strings and the host texts (`errorText`), and
            // some panels act on the code (`plugin.coverage.errors.none`); the status bar and
            // MCP get the sentence. RB-080-90: and the log gets the failure.
            let value = value.map_err(|failure| {
                super::panel_errors::log_failure(&plugin, &name, &failure);
                (IpcErrorCode::Internal, failure_text(&failure))
            })?;
            // RB-091-02: the panel's "Open report", its history's delete, the run's retention.
            let value = crate::report_view::take(core, &runtime, &plugin, value);
            Ok(json!({ "value": value }))
        }
        PanelRequest::Selected { plugin, panel } => {
            crate::panel_dock::selected(core, &plugin, &panel)
                .map_err(|reason| (IpcErrorCode::BadRequest, reason))?;
            Ok(json!({ "value": null }))
        }
        // The plugin catalogue (RB-091-01): the network and the disk on a blocking thread.
        PanelRequest::Catalog { refresh } => {
            let user = core.user_settings();
            blocking(move || {
                let dir = super::catalog::store_dir().ok_or_else(|| {
                    (
                        IpcErrorCode::Internal,
                        "plugins.catalog.error.io".to_owned(),
                    )
                })?;
                let fetcher = rb_plugin_host::HttpFetcher::new();
                let fetcher: Option<&dyn rb_plugin_host::Fetcher> =
                    if refresh { Some(&fetcher) } else { None };
                let installed = super::catalog::installed_now();
                let mut answer = super::catalog::answer(&user, dir, fetcher, installed);
                // RB-091-04: the bundle's plugins, each with its update.
                if let Value::Object(fields) = &mut answer {
                    fields.insert(
                        "bundled".to_owned(),
                        Value::Array(super::bundle_offer::bundled_rows(&user)),
                    );
                }
                Ok(answer)
            })
            .await
        }
        // The first-run wizard's plugins (RB-091-04, RB-091-05).
        PanelRequest::Available => {
            let core = core.clone();
            blocking(move || Ok(super::bundle_offer::available(&core))).await
        }
        PanelRequest::InstallChosen { ids } => {
            let core = core.clone();
            blocking(move || super::bundle_install::install_chosen(&core, ids)).await
        }
        PanelRequest::RemoveChosen { ids } => {
            let core = core.clone();
            blocking(move || super::bundle_install::remove_chosen(&core, ids)).await
        }
        PanelRequest::ApplyRecommended => {
            let core = core.clone();
            blocking(move || super::bundle_install::apply_recommended(&core)).await
        }
        PanelRequest::CatalogPrepare {
            repository,
            id,
            version,
        } => {
            let core = core.clone();
            blocking(move || super::catalog::review(&core, &repository, &id, &version)).await
        }
        PanelRequest::CatalogInstall {
            repository,
            id,
            version,
            consent,
        } => {
            let core = core.clone();
            let request = super::catalog::InstallRequest {
                repository,
                id,
                version,
                consent,
            };
            blocking(move || super::catalog::install(&core, request)).await
        }
        PanelRequest::CatalogRemove { id } => {
            let core = core.clone();
            blocking(move || super::catalog::remove(&core, &id)).await
        }
        PanelRequest::SelectedCore { panel } => {
            crate::panel_dock::core_panels::selected(core, &panel)
                .map_err(|reason| (IpcErrorCode::BadRequest, reason))?;
            Ok(json!({ "value": null }))
        }
        PanelRequest::Settings { plugin } => {
            let runtime = runtime_of(core)?;
            has_panels(core, &runtime, &plugin)?;
            // RB-080-90: with the host codes' texts, so a panel says a failure as a sentence.
            let mut strings = super::panel_errors::host_texts();
            strings.extend(rb_i18n::plugin_texts(&plugin));
            Ok(json!({
                "strings": strings,
                "settings": with_theme(
                    with_shell_keys(
                        super::services::shared().plugin_values(&plugin),
                        &core.user_settings(),
                    ),
                    core.user_settings().ui.theme,
                ),
            }))
        }
    }
}

/// `work` on a blocking thread: a catalogue step reaches the network and the disk.
async fn blocking(
    work: impl FnOnce() -> Result<Value, Failure> + Send + 'static,
) -> Result<Value, Failure> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| (IpcErrorCode::Internal, error.to_string()))?
}

/// The plugin runtime of `core`'s window, once it started.
fn runtime_of(core: &Core) -> Result<Arc<PluginRuntime>, Failure> {
    for_host(core.host()).ok_or_else(|| {
        (
            IpcErrorCode::NotFound,
            "plugins.error.not_available".to_owned(),
        )
    })
}

/// The installed plugins and the folders the start read (RB-080-51). A request before the load
/// started starts it, on its own thread; the answer never waits for it.
async fn installed(core: &Arc<Core>) -> Result<Value, Failure> {
    crate::analyzer::plugins::ensure_loading(core.runtime().clone());
    let mut answer = super::list::answer(super::list::current());
    // The folders are looked at on a blocking thread.
    let folders = tokio::task::spawn_blocking(super::list::folders)
        .await
        .map_err(|error| (IpcErrorCode::Internal, error.to_string()))?;
    if let (Value::Object(fields), Value::Object(folders)) = (&mut answer, folders) {
        fields.extend(folders);
    }
    Ok(answer)
}

/// The panels offered now: the served plugins' but the hidden ones and, without workspace
/// trust, a project plugin's.
pub(crate) fn panels_offered(
    core: &Core,
    runtime: &PluginRuntime,
) -> Vec<super::panels::PanelInfo> {
    panels_offered_for(&core.user_settings(), runtime)
}

/// [`panels_offered`] for the user settings `user` a caller holds already.
pub(crate) fn panels_offered_for(
    user: &rb_config::UserSettings,
    runtime: &PluginRuntime,
) -> Vec<super::panels::PanelInfo> {
    let hidden: BTreeSet<String> = user
        .plugins
        .panels_hidden
        .iter()
        .map(|id| id.0.clone())
        .collect();
    let plugins = crate::analyzer::plugins::bundled();
    let served = plugins
        .report
        .loaded
        .iter()
        .map(|entry| entry.plugin.manifest())
        .filter(|manifest| runtime.serves(&manifest.id));
    panels_of(served, &hidden, rb_i18n::t)
}

/// Only a plugin with an offered panel takes a panel's call.
fn has_panels(core: &Core, runtime: &PluginRuntime, plugin: &str) -> Result<(), Failure> {
    if panels_offered(core, runtime)
        .iter()
        .any(|panel| panel.plugin == plugin)
    {
        Ok(())
    } else {
        Err((
            IpcErrorCode::NotFound,
            "plugins.error.unknown_contribution".to_owned(),
        ))
    }
}

/// The settings a plugin's panel gets: the effective values of the plugin's own keys,
/// `<plugin-id>.<property>` (the user's value or its schema default), and the shell's keys every
/// panel follows - `ui.syntaxHighlight` for the code it shows (RB-080-65) and
/// `plugins.panelMessageMaxBytes`, the message limit the shell's bridge holds and a panel sizes
/// its answers by (RB-090-44).
pub(crate) fn with_shell_keys(
    mut values: Map<String, Value>,
    user: &rb_config::UserSettings,
) -> Value {
    values.insert(
        "ui.syntaxHighlight".to_owned(),
        Value::Bool(user.ui.syntax_highlight),
    );
    values.insert(
        super::panel_limit::SETTING.to_owned(),
        Value::from(user.plugins.panel_message_limit()),
    );
    Value::Object(values)
}

#[cfg(test)]
#[path = "ipc_tests.rs"]
mod tests;
