//! The window's [`ApiHost`] (RB-096-01): each op takes its [`Context`] from the window - the
//! project root, the workspace trust, the environment the views show, the project's
//! environments, the plugin's settings - and runs on a blocking thread's time; a host the
//! person allowed is written to `config.local.json` (`api-client.allowedHosts`) through the
//! settings window's own path, as the settings window writes it. A collection run's failures go
//! to the findings panel as the plugin's own run (RB-096-04, `crate::checks::reported`).
//!
//! A capture (RB-096-08, [`super::capture_ops`]) is gathered here: each request's headers as sent
//! from the views' buffers (`crate::plugins::console`), its post data from its view
//! (`Network.getRequestPostData`, a body it cannot hand over said as `unread`); the values kept
//! go to `api-client.secrets` the same way (and so do the Variables tab's private rows and a value
//! set for a `$secret`, RB-096-11) - when that write fails the answer stands and says so
//! ([`capture_ops::not_kept`]) -, and an open's draft waits for the panel's next `state` while
//! the panels show the API client.

use std::sync::{Arc, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rb_engine::{ViewHost as _, ViewId};
use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::{Value, json};

use super::call::{ApiHost, set_host};
use super::capture::{Captured, body_expected};
use super::capture_ops::{self, SecretMode, Wanted};
use super::ops::{self, Context, Sender, Settings};
use super::protocol::{CaptureRef, Op, Origin};
use super::send::{self, Exchange, Prepared, Route};
use super::variables;
use super::{
    BENCH_CONCURRENCY_KEY, BENCH_REPEATS_KEY, DEFAULT_HISTORY_SIZE, DEFAULT_MAX_RESPONSE_BYTES,
    HISTORY_SIZE_KEY, MAX_RESPONSE_BYTES, MAX_RESPONSE_KEY, PLUGIN_ID, SECRETS_KEY, bench, error,
    grants,
};
use crate::control::Core;

/// Sends for real: the environment's proxy, the network.
struct NetworkSender;

impl Sender for NetworkSender {
    fn send(
        &self,
        prepared: &Prepared,
        limit: u64,
        secrets: &[String],
    ) -> Result<Exchange, Failure> {
        send::send_blocking(prepared, limit, secrets, Route::Environment)
    }
}

fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// The ops of one window.
pub struct WindowApi {
    core: Weak<Core>,
}

/// Starts the API client's host for `core`'s window.
pub fn start(core: &Core) {
    let Some(core) = core.arc() else {
        return;
    };
    set_host(Arc::new(WindowApi {
        core: Arc::downgrade(&core),
    }));
}

/// The settings values in effect, held to their ranges.
#[must_use]
pub fn settings_of(values: &serde_json::Map<String, Value>) -> Settings {
    let history_size =
        values
            .get(HISTORY_SIZE_KEY)
            .and_then(Value::as_u64)
            .map_or(DEFAULT_HISTORY_SIZE, |size| {
                usize::try_from(size.min(super::history::MAX_ENTRIES as u64))
                    .unwrap_or(DEFAULT_HISTORY_SIZE)
            });
    let max_response_bytes = values
        .get(MAX_RESPONSE_KEY)
        .and_then(Value::as_u64)
        .map_or(DEFAULT_MAX_RESPONSE_BYTES, |bytes| {
            bytes.clamp(1024, MAX_RESPONSE_BYTES)
        });
    let small = |key: &str, default: u32, most: u32| {
        values
            .get(key)
            .and_then(Value::as_u64)
            .map_or(default, |n| {
                u32::try_from(n.clamp(1, u64::from(most))).unwrap_or(most)
            })
    };
    Settings {
        history_size,
        max_response_bytes,
        bench_repeats: small(
            BENCH_REPEATS_KEY,
            bench::DEFAULT_REPEATS,
            bench::MAX_REPEATS,
        ),
        bench_concurrency: small(BENCH_CONCURRENCY_KEY, 1, bench::MAX_CONCURRENCY),
        code: super::export::settings_of(values),
    }
}

fn context(core: &Core) -> Context {
    let host = core.host();
    let config = host.config();
    let state = host.shell_state();
    let values = crate::plugins::services::shared().plugin_values(PLUGIN_ID);
    Context {
        root: host.project_root(),
        trusted: state.trusted,
        environment: state.environment.clone(),
        environments: config
            .environments
            .iter()
            .map(|environment| (environment.id.clone(), environment.url.clone()))
            .collect(),
        allowed_hosts: grants::list_of(values.get(grants::ALLOWED_HOSTS_KEY)),
        secrets: values.get(SECRETS_KEY).cloned().unwrap_or(Value::Null),
        settings: settings_of(&values),
        now: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
    }
}

/// Writes `hosts` as the project's grants, as the settings window does.
fn keep_hosts(core: &Core, hosts: &[String]) -> Result<(), Failure> {
    let answer = crate::settings_window::set_setting(
        core,
        grants::ALLOWED_HOSTS_KEY,
        &json!(hosts),
        rb_settings::Scope::Local,
        crate::settings_window::Caller::Shell,
    );
    if matches!(answer, rb_ipc::CoreToShell::SettingRejected { .. }) {
        return Err(error(FailureKind::Io, "grant_not_saved"));
    }
    crate::plugins::services::shared().settings_changed();
    tracing::info!("a host was allowed for the API client");
    Ok(())
}

/// Writes `secrets` as the private values, as the settings window does.
fn keep_secrets(core: &Core, secrets: &Value) -> Result<(), Failure> {
    let answer = crate::settings_window::set_setting(
        core,
        SECRETS_KEY,
        secrets,
        rb_settings::Scope::Local,
        crate::settings_window::Caller::Shell,
    );
    if matches!(answer, rb_ipc::CoreToShell::SettingRejected { .. }) {
        return Err(error(FailureKind::Io, "secrets_not_saved"));
    }
    crate::plugins::services::shared().settings_changed();
    tracing::info!("private values were kept for the API client");
    Ok(())
}

/// What the views hold of request `one`: `None` once they no longer list it.
async fn gathered(core: &Core, one: &CaptureRef) -> Option<Captured> {
    let view = ViewId(one.view);
    let entry = crate::plugins::console::request(view, &one.id)?;
    let headers = entry.request_headers().to_vec();
    let sends_body = !matches!(entry.method.as_str(), "GET" | "HEAD" | "OPTIONS");
    let body = if sends_body {
        core.host()
            .cdp_send(
                view,
                "Network.getRequestPostData",
                json!({ "requestId": one.id }),
            )
            .await
            .ok()
            .and_then(|answer| answer["postData"].as_str().map(str::to_owned))
    } else {
        None
    };
    let body_unread = sends_body && body.is_none() && body_expected(&entry.method, &headers);
    Some(Captured {
        method: entry.method.clone(),
        url: entry.url.clone(),
        headers,
        body,
        body_unread,
    })
}

/// The op `capture` for a caller `origin`: only the panel keeps values (as the setting
/// `api-client.captureSecrets` says) and opens the editor.
async fn capture(
    core: Arc<Core>,
    requests: Vec<CaptureRef>,
    save: bool,
    open: bool,
    origin: Origin,
) -> Result<Value, Failure> {
    if requests.len() > capture_ops::MAX_CAPTURES {
        return Err(error(FailureKind::Limit, "too_many_captures")
            .with_param("most", capture_ops::MAX_CAPTURES.to_string()));
    }
    let mut captured = Vec::with_capacity(requests.len());
    for one in &requests {
        captured.push(gathered(&core, one).await);
    }
    let panel = origin == Origin::Panel;
    let working = Arc::clone(&core);
    let (answer, opened) = tokio::task::spawn_blocking(move || {
        let ctx = context(&working);
        let values = crate::plugins::services::shared().plugin_values(PLUGIN_ID);
        let wanted = Wanted {
            save,
            open: open && panel,
            keep: panel && capture_ops::mode_of(&values) == SecretMode::Private,
        };
        let mut taken = capture_ops::run(&ctx, captured, wanted)?;
        if let Some(secrets) = taken.secrets.take()
            && keep_secrets(&working, &secrets).is_err()
        {
            tracing::warn!("the captured values were not kept; the answer says so");
            capture_ops::not_kept(&mut taken);
        }
        let opened = taken.draft.is_some();
        if let Some(draft) = taken.draft {
            capture_ops::put_draft(draft);
        }
        Ok::<_, Failure>((taken.answer, opened))
    })
    .await
    .map_err(|_| Failure::coded(FailureKind::Other, codes::HOST_FAILED))??;
    if opened {
        crate::plugins::commands::open_panels_unfolded(
            &core,
            &json!({ "plugin": PLUGIN_ID, "panel": PLUGIN_ID }),
        );
        core.send(&rb_ipc::CoreToShell::PluginPanel {
            message: crate::plugins::panels::event_message(
                PLUGIN_ID,
                json!({ "message": { "draft": true } }),
            ),
        });
    }
    Ok(answer)
}

#[async_trait]
impl ApiHost for WindowApi {
    async fn run(&self, op: Op, origin: Origin) -> Result<Value, Failure> {
        let core = self.core.upgrade().ok_or_else(|| {
            Failure::coded(FailureKind::NotFound, codes::NOT_AVAILABLE)
                .with_param("service", "api client")
        })?;
        tracing::debug!(op = op.id(), origin = origin.id(), "an API client op");
        let op = match op {
            Op::Capture {
                requests,
                save,
                open,
            } => return capture(core, requests, save, open, origin).await,
            other => other,
        };
        // The panel's `state` takes the draft an open left for it (RB-096-08).
        let draft = origin == Origin::Panel && matches!(op, Op::State);
        let mut answer = tokio::task::spawn_blocking(move || {
            let ctx = context(&core);
            let outcome = ops::run(op, &ctx, &NetworkSender, &process_env)?;
            if let Some(hosts) = &outcome.allowed_hosts {
                keep_hosts(&core, hosts)?;
            }
            // RB-096-11: the Variables tab's private rows, a value set for a `$secret`; the
            // tab's file only after them, and the values as they were again when it fails.
            let previous = if ctx.secrets.is_object() {
                ctx.secrets.clone()
            } else {
                json!({})
            };
            variables::commit(
                ctx.root.as_deref(),
                &previous,
                outcome.secrets.as_ref(),
                outcome.env_file.as_ref(),
                |secrets| keep_secrets(&core, secrets),
            )?;
            if let Some(findings) = outcome.findings {
                crate::checks::reported::publish(&core, PLUGIN_ID, findings);
            }
            Ok::<_, Failure>(outcome.answer)
        })
        .await
        .map_err(|_| Failure::coded(FailureKind::Other, codes::HOST_FAILED))??;
        if draft
            && let Value::Object(fields) = &mut answer
            && let Some(waiting) = capture_ops::take_draft()
        {
            fields.insert("draft".to_owned(), waiting);
        }
        Ok(answer)
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
