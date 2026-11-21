//! The window's [`LoadHost`] (RB-096-07): each op takes its [`Context`] from the window - the
//! project root, the workspace trust, `startUrl` and the environments, the API client's private
//! values and settings, the load test's ceilings and confirmed hosts - and runs on a blocking
//! thread's time. A confirmed host is written to `config.local.json`
//! (`load-test.allowedHosts`) through the settings window's own path. One run at a time, on the
//! core's runtime: it pushes its snapshot to the plugin's panels after every second
//! ([`super::snapshot`]), shows in the activity bar as the plugin's progress with its Stop
//! (the plugin's `cancel` command), keeps its report at the end, and ends with the window
//! ([`stop_all`], called when the main window closes).

use std::sync::{Arc, Mutex, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rb_ipc::CoreToShell;
use rb_plugin_host::HostServices as _;
use rb_plugin_types::{Failure, FailureKind, codes};
use rb_util::lock;
use serde_json::{Value, json};

use super::call::{LoadHost, set_host};
use super::engine::{End, Halt, OnSecond, Tick, run};
use super::gate::ALLOWED_HOSTS_KEY;
use super::ops::{self, RunState};
use super::plan::{Context, Launch};
use super::profile::Limits;
use super::protocol::{Op, Origin};
use super::report::{Reports, build};
use super::report_html::render;
use super::shot::LiveShot;
use super::snapshot::snapshot;
use super::{PLUGIN_ID, error};
use crate::api_client::grants::list_of;
use crate::control::Core;

/// The run of this process: running or the last one.
struct Current {
    halt: Halt,
    running: bool,
    snapshot: Option<Value>,
}

static CURRENT: Mutex<Option<Current>> = Mutex::new(None);

fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Ends the running run, if one runs, with `end`; whether one did.
pub fn stop_all(end: End) -> bool {
    lock(&CURRENT)
        .as_ref()
        .is_some_and(|current| current.running && current.halt.halt(end))
}

fn run_state() -> RunState {
    lock(&CURRENT)
        .as_ref()
        .map(|current| RunState {
            running: current.running,
            snapshot: current.snapshot.clone(),
        })
        .unwrap_or_default()
}

/// The ops of one window.
pub struct WindowLoad {
    core: Weak<Core>,
}

/// Starts the load test's host for `core`'s window.
pub fn start(core: &Core) {
    let Some(core) = core.arc() else {
        return;
    };
    set_host(Arc::new(WindowLoad {
        core: Arc::downgrade(&core),
    }));
}

fn context(core: &Core) -> Context {
    let host = core.host();
    let config = host.config();
    let state = host.shell_state();
    let services = crate::plugins::services::shared();
    let api_values = services.plugin_values(crate::api_client::PLUGIN_ID);
    let values = services.plugin_values(PLUGIN_ID);
    Context {
        api: crate::api_client::ops::Context {
            root: host.project_root(),
            trusted: state.trusted,
            environment: state.environment.clone(),
            environments: config
                .environments
                .iter()
                .map(|environment| (environment.id.clone(), environment.url.clone()))
                .collect(),
            // The run's own hosts replace these (`plan::launch`).
            allowed_hosts: Vec::new(),
            secrets: api_values
                .get(crate::api_client::SECRETS_KEY)
                .cloned()
                .unwrap_or(Value::Null),
            settings: crate::api_client::window::settings_of(&api_values),
            now: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
        },
        start_url: config.start_url.clone(),
        limits: Limits::of(&values),
        confirmed: list_of(values.get(ALLOWED_HOSTS_KEY)),
    }
}

/// Writes `hosts` as the project's confirmed hosts, as the settings window does.
fn keep_hosts(core: &Core, hosts: &[String]) -> Result<(), Failure> {
    let answer = crate::settings_window::set_setting(
        core,
        ALLOWED_HOSTS_KEY,
        &json!(hosts),
        rb_settings::Scope::Local,
        crate::settings_window::Caller::Shell,
    );
    if matches!(answer, CoreToShell::SettingRejected { .. }) {
        return Err(error(FailureKind::Io, "grant_not_saved"));
    }
    crate::plugins::services::shared().settings_changed();
    tracing::info!("a host was confirmed for the load test");
    Ok(())
}

fn tell_panels(core: &Core, snapshot: &Value) {
    let event = json!({ "message": { "loadTest": snapshot } });
    core.send(&CoreToShell::PluginPanel {
        message: crate::plugins::panels::event_message(PLUGIN_ID, event),
    });
}

fn keep_snapshot(snapshot: &Value, running: bool) {
    if let Some(current) = lock(&CURRENT).as_mut() {
        current.snapshot = Some(snapshot.clone());
        current.running = running;
    }
}

/// Starts `launch` on the core's runtime.
fn begin(core: &Arc<Core>, launch: Launch) -> Result<(), Failure> {
    let halt = Halt::default();
    {
        let mut current = lock(&CURRENT);
        if current.as_ref().is_some_and(|current| current.running) {
            return Err(error(FailureKind::Limit, "already_running"));
        }
        *current = Some(Current {
            halt: halt.clone(),
            running: true,
            snapshot: None,
        });
    }
    let Launch {
        plan, live, meta, ..
    } = launch;
    let root = live.ctx.root.clone();
    let services = crate::plugins::services::shared();
    // The activity bar's entry, with the plugin's cancel command as its Stop (RB-093-01).
    services.call_started(PLUGIN_ID);
    let total = u32::try_from(meta.profile.duration_s()).unwrap_or(u32::MAX);
    tracing::info!(
        users = meta.profile.users,
        duration_s = total,
        targets = meta.targets.len(),
        "a load test started"
    );
    let weak = Arc::downgrade(core);
    let seen = meta.clone();
    let on_second: OnSecond = Box::new(move |recorder, tick| {
        let live = snapshot(&seen, recorder, tick, None, None);
        keep_snapshot(&live, true);
        crate::plugins::services::shared().report_progress(PLUGIN_ID, tick.t.min(total), total);
        if let Some(core) = weak.upgrade() {
            tell_panels(&core, &live);
        }
    });
    let weak = Arc::downgrade(core);
    core.runtime().spawn(async move {
        let finished = run(plan, Arc::new(LiveShot(Arc::new(live))), halt, on_second).await;
        let report = build(&meta, &finished);
        let page = render(&report);
        let saved = match root {
            Some(root) => {
                tokio::task::spawn_blocking(move || Reports::new(&root).save(&report, &page))
                    .await
                    .ok()
                    .and_then(Result::ok)
            }
            None => None,
        };
        let totals = finished.recorder.totals(finished.elapsed_s);
        let tick = Tick {
            t: finished.recorder.series().last().map_or(0, |point| point.t),
            elapsed_s: finished.elapsed_s,
            users: 0,
            target: 0,
        };
        let last = snapshot(
            &meta,
            &finished.recorder,
            &tick,
            Some(finished.end),
            saved.as_deref(),
        );
        keep_snapshot(&last, false);
        crate::plugins::services::shared().call_ended(PLUGIN_ID);
        tracing::info!(
            end = finished.end.id(),
            requests = totals.requests,
            errors = totals.errors,
            "a load test ended"
        );
        if let Some(core) = weak.upgrade() {
            tell_panels(&core, &last);
        }
    });
    Ok(())
}

#[async_trait]
impl LoadHost for WindowLoad {
    async fn run(&self, op: Op, origin: Origin) -> Result<Value, Failure> {
        let core = self.core.upgrade().ok_or_else(|| {
            Failure::coded(FailureKind::NotFound, codes::NOT_AVAILABLE)
                .with_param("service", "load test")
        })?;
        tracing::debug!(op = op.id(), origin = origin.id(), "a load test op");
        if matches!(op, Op::OpenApiClient) {
            ops::api_client_reachable(core.has_command(ops::API_CLIENT_PANEL_COMMAND))?;
        }
        let state = run_state();
        let working = Arc::clone(&core);
        let outcome = tokio::task::spawn_blocking(move || {
            ops::run(op, &context(&working), &state, &process_env)
        })
        .await
        .map_err(|_| Failure::coded(FailureKind::Other, codes::HOST_FAILED))??;
        if let Some(hosts) = &outcome.confirmed {
            keep_hosts(&core, hosts)?;
        }
        if outcome.stop {
            stop_all(End::Stopped);
        }
        if let Some(launch) = outcome.launch {
            begin(&core, launch)?;
        }
        if outcome.open_api_client {
            // The API client's own command (RB-096-09), off this task: a plugin command. Its
            // failure after the answer goes to the status bar, as a shortcut's does.
            let weak = Arc::downgrade(&core);
            core.run_command_async(ops::API_CLIENT_PANEL_COMMAND, Value::Null, move |result| {
                if let Err(error) = result {
                    tracing::warn!(%error, "the API client panel did not open");
                    if let Some(core) = weak.upgrade() {
                        let title = core.command_title(ops::API_CLIENT_PANEL_COMMAND);
                        core.notify(crate::control::failure_notification(&title, &error));
                    }
                }
            });
        }
        Ok(outcome.answer)
    }
}
