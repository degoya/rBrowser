//! The linker: `host` always, `settings` when declared, and one interface per granted
//! capability - nothing else, so a component importing more fails to instantiate even if it got
//! past the import check.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host/src/link.rs`: linked by hand under
//! the versioned names rather than through the generated `add_to_linker`, which would link every
//! interface of the world. Every function hands its work to the plugin's [`PluginContext`], which
//! checks the grants again, and credits the time it took back to the call.

use std::{future::Future, sync::Arc, time::Instant};

use rb_plugin_api::versioned;
use rb_plugin_types::{Capability, Failure, FailureKind, Finding, HttpRequest, Severity};
use wasmtime::{StoreContextMut, component::Linker};

use crate::{
    bindings::rbrowser::plugin::{http, types, views},
    context::PluginContext,
    failure::readable,
    manifest::PluginManifest,
    runtime::PluginStoreState,
};

type Context<'a> = StoreContextMut<'a, PluginStoreState>;
type WitResult<T> = Result<T, types::Failure>;

/// Longest plugin log line kept, in characters.
const MAX_LOG_CHARS: usize = 4096;

/// Links what `manifest` may import.
pub(crate) fn link(
    linker: &mut Linker<PluginStoreState>,
    manifest: &PluginManifest,
) -> wasmtime::Result<()> {
    link_host(linker)?;
    if manifest.settings {
        link_settings(linker)?;
    }
    for capability in &manifest.grants.granted {
        match capability {
            Capability::Views => link_views(linker)?,
            Capability::Evaluate => link_evaluate(linker)?,
            Capability::Cdp => link_cdp(linker)?,
            Capability::Http => link_http(linker)?,
            Capability::ProjectRead => link_project(linker)?,
            Capability::OutWrite => link_out(linker)?,
            Capability::Findings => link_findings(linker)?,
            Capability::Sidecar => link_sidecar(linker)?,
            Capability::NetworkRules | Capability::ConsoleLogs | Capability::Status => {}
        }
    }
    Ok(())
}

/// The WIT record of a host failure.
pub(crate) fn to_wit_failure(failure: Failure) -> types::Failure {
    types::Failure {
        kind: match failure.kind {
            FailureKind::NotFound => types::FailureKind::NotFound,
            FailureKind::AccessDenied => types::FailureKind::AccessDenied,
            FailureKind::NotSupported => types::FailureKind::NotSupported,
            FailureKind::BadData => types::FailureKind::BadData,
            FailureKind::Cancelled => types::FailureKind::Cancelled,
            FailureKind::Io => types::FailureKind::Io,
            FailureKind::Limit => types::FailureKind::Limit,
            FailureKind::Other => types::FailureKind::Other,
        },
        code: failure.code,
        params: failure.params.into_iter().collect(),
    }
}

/// Runs one host function on the plugin's context and credits its time back to the call.
async fn timed<T, F>(
    store: &mut Context<'_>,
    work: impl FnOnce(Arc<PluginContext>) -> F,
) -> WitResult<T>
where
    F: Future<Output = Result<T, Failure>>,
{
    let context = Arc::clone(&store.data().context);
    let started = Instant::now();
    let answer = work(context).await;
    store.data_mut().credit_host_time(started.elapsed());
    answer.map_err(to_wit_failure)
}

fn link_host(linker: &mut Linker<PluginStoreState>) -> wasmtime::Result<()> {
    let mut instance = linker.instance(&versioned("host"))?;
    instance.func_wrap_async::<(String, String), (), _>("log", |mut store, (level, message)| {
        Box::new(async move {
            log(store.data_mut(), &level, &message);
            Ok(())
        })
    })?;
    instance.func_wrap_async::<(), (u64,), _>("now-unix-seconds", |_store, ()| {
        Box::new(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs())
                .unwrap_or_default();
            Ok((now,))
        })
    })?;
    instance.func_wrap_async::<(u32, u32), (), _>("report-progress", |store, (done, total)| {
        Box::new(async move {
            store.data().context.report_progress(done, total);
            Ok(())
        })
    })?;
    Ok(())
}

/// At most [`crate::MAX_LOG_LINES_PER_CALL`] lines per call reach the log, each cut to
/// [`MAX_LOG_CHARS`] with its control and direction characters spelled out, so a plugin can
/// neither flood the log nor forge a line of rBrowser's.
fn log(state: &mut PluginStoreState, level: &str, message: &str) {
    if !state.log_budget.admit() {
        return;
    }
    let line = readable(message, MAX_LOG_CHARS);
    let origin = state.log_budget.origin();
    let plugin_id = origin.plugin_id.as_str();
    let operation = origin.operation;
    match level.to_ascii_lowercase().as_str() {
        "error" => tracing::error!(plugin = true, plugin_id, operation, "{line}"),
        "warn" => tracing::warn!(plugin = true, plugin_id, operation, "{line}"),
        "debug" | "trace" => tracing::debug!(plugin = true, plugin_id, operation, "{line}"),
        _ => tracing::info!(plugin = true, plugin_id, operation, "{line}"),
    }
}

fn link_settings(linker: &mut Linker<PluginStoreState>) -> wasmtime::Result<()> {
    let mut instance = linker.instance(&versioned("settings"))?;
    instance.func_wrap_async::<(String,), (Option<String>,), _>("get", |store, (key,)| {
        Box::new(async move { Ok((store.data().context.setting(&key),)) })
    })?;
    Ok(())
}

fn link_views(linker: &mut Linker<PluginStoreState>) -> wasmtime::Result<()> {
    let mut instance = linker.instance(&versioned("views"))?;
    instance.func_wrap_async::<(), (Vec<views::ViewInfo>,), _>("list", |store, ()| {
        Box::new(async move {
            let list = store
                .data()
                .context
                .views()
                .into_iter()
                .map(|view| views::ViewInfo {
                    id: view.id,
                    label: view.label,
                    breakpoint: view.breakpoint,
                    engine: view.engine,
                    width: view.width,
                    height: view.height,
                    dpr: view.dpr,
                    url: view.url,
                    title: view.title,
                })
                .collect();
            Ok((list,))
        })
    })?;
    instance.func_wrap_async::<(), (Option<u32>,), _>("active", |store, ()| {
        Box::new(async move { Ok((store.data().context.active_view(),)) })
    })?;
    Ok(())
}

fn link_evaluate(linker: &mut Linker<PluginStoreState>) -> wasmtime::Result<()> {
    let mut instance = linker.instance(&versioned("evaluate"))?;
    instance.func_wrap_async::<(u32, String), (WitResult<String>,), _>(
        "evaluate",
        |mut store, (view, script)| {
            Box::new(async move {
                let answer = timed(&mut store, |context| async move {
                    context.evaluate(view, &script).await
                })
                .await;
                Ok((answer,))
            })
        },
    )?;
    Ok(())
}

fn link_cdp(linker: &mut Linker<PluginStoreState>) -> wasmtime::Result<()> {
    let mut instance = linker.instance(&versioned("cdp"))?;
    instance.func_wrap_async::<(u32, String, String), (WitResult<String>,), _>(
        "send",
        |mut store, (view, method, params)| {
            Box::new(async move {
                let answer = timed(&mut store, |context| async move {
                    context.cdp_send(view, &method, &params).await
                })
                .await;
                Ok((answer,))
            })
        },
    )?;
    Ok(())
}

fn link_http(linker: &mut Linker<PluginStoreState>) -> wasmtime::Result<()> {
    type Request = (String, String, Vec<(String, String)>, Vec<u8>);
    let mut instance = linker.instance(&versioned("http"))?;
    instance.func_wrap_async::<Request, (WitResult<http::HttpResponse>,), _>(
        "fetch",
        |mut store, (method, url, headers, body)| {
            Box::new(async move {
                let request = HttpRequest {
                    method,
                    url,
                    headers,
                    body,
                };
                let answer = timed(
                    &mut store,
                    |context| async move { context.fetch(request).await },
                )
                .await
                .map(|response| http::HttpResponse {
                    status: response.status,
                    headers: response.headers,
                    body: response.body,
                });
                Ok((answer,))
            })
        },
    )?;
    Ok(())
}

fn link_project(linker: &mut Linker<PluginStoreState>) -> wasmtime::Result<()> {
    let mut instance = linker.instance(&versioned("project"))?;
    instance.func_wrap_async::<(String,), (WitResult<Vec<u8>>,), _>(
        "read",
        |mut store, (path,)| {
            Box::new(async move {
                let answer = timed(&mut store, |context| async move {
                    context.project_read(&path).await
                })
                .await;
                Ok((answer,))
            })
        },
    )?;
    instance.func_wrap_async::<(String,), (WitResult<Vec<String>>,), _>(
        "list",
        |mut store, (dir,)| {
            Box::new(async move {
                let answer = timed(&mut store, |context| async move {
                    context.project_list(&dir).await
                })
                .await;
                Ok((answer,))
            })
        },
    )?;
    Ok(())
}

fn link_out(linker: &mut Linker<PluginStoreState>) -> wasmtime::Result<()> {
    let mut instance = linker.instance(&versioned("out"))?;
    instance.func_wrap_async::<(String, Vec<u8>), (WitResult<()>,), _>(
        "write",
        |mut store, (path, bytes)| {
            Box::new(async move {
                let answer = timed(&mut store, |context| async move {
                    context.out_write(&path, &bytes).await
                })
                .await;
                Ok((answer,))
            })
        },
    )?;
    Ok(())
}

fn link_findings(linker: &mut Linker<PluginStoreState>) -> wasmtime::Result<()> {
    let mut instance = linker.instance(&versioned("findings"))?;
    instance.func_wrap_async::<(Vec<types::Finding>,), (WitResult<()>,), _>(
        "report",
        |store, (items,)| {
            Box::new(async move {
                let items = items.into_iter().map(from_wit_finding).collect();
                Ok((store
                    .data()
                    .context
                    .report_findings(items)
                    .map_err(to_wit_failure),))
            })
        },
    )?;
    Ok(())
}

fn link_sidecar(linker: &mut Linker<PluginStoreState>) -> wasmtime::Result<()> {
    let mut instance = linker.instance(&versioned("sidecar"))?;
    instance.func_wrap_async::<(String, String), (WitResult<String>,), _>(
        "call",
        |mut store, (method, params)| {
            Box::new(async move {
                let answer = timed(&mut store, |context| async move {
                    context.sidecar_call(&method, &params).await
                })
                .await;
                Ok((answer,))
            })
        },
    )?;
    Ok(())
}

/// A WIT severity as the host holds it.
pub(crate) fn from_wit_severity(severity: types::Severity) -> Severity {
    match severity {
        types::Severity::Info => Severity::Info,
        types::Severity::Warning => Severity::Warning,
        types::Severity::Error => Severity::Error,
    }
}

/// A WIT finding as the host holds it.
pub(crate) fn from_wit_finding(finding: types::Finding) -> Finding {
    Finding {
        rule: finding.rule,
        severity: from_wit_severity(finding.severity),
        message: finding.message,
        params: finding.params,
        view: finding.view,
        url: finding.url,
        selector: finding.selector,
        path: finding.path,
        data: finding.data,
    }
}
