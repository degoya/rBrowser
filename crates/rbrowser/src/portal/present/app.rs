//! The presentation in the app (RB-096-05): starting it - setting, consent, cloudflared, the
//! listener on loopback, the tunnel - following the tunnel until it is live, a new link, the
//! stop, and the glue to the grid (the presentation's listener follows the active view like the
//! LAN portal). One presentation per process. It never starts by itself: a person's
//! `portal.present.start` starts it, never MCP; it ends with its stop, the LAN portal's stop, a
//! change of its settings, a crash of the tunnel and rBrowser's end.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, SystemTime};

use rb_commands::CommandError;
use rb_config::PresentationSettings;
use rb_engine::ViewHost as _;
use rb_ipc::{CoreToShell, NotificationLevel};
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedReceiver;

use super::cloudflared;
use super::notice::{self, notice};
use super::paths::{PathRules, base_path};
use super::public::Public;
use super::tunnel::{Tunnel, TunnelEvent};
use crate::cdp::lock;
use crate::control::Core;
use crate::portal::PORTAL_AGENT_SOURCE;
use crate::portal::server::{Options, PortalServer};

/// How long the tunnel may take to connect.
const START_TIMEOUT: Duration = Duration::from_secs(60);

/// Where a presentation stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The tunnel opens.
    Starting,
    /// Viewers can join.
    Live,
}

/// The running presentation.
pub(super) struct Running {
    pub(super) server: Arc<PortalServer>,
    pub(super) public: Arc<Public>,
    tunnel: Option<Tunnel>,
    pub(super) phase: Phase,
    /// The last link issued (it holds the token: the panel only) and when it runs out.
    pub(super) link: Option<(String, SystemTime)>,
    started_with: PresentationSettings,
    pub(super) started_at: SystemTime,
    generation: u64,
}

pub(super) static STATE: Mutex<Option<Running>> = Mutex::new(None);
/// cloudflared is being downloaded for a start.
pub(super) static DOWNLOADING: AtomicBool = AtomicBool::new(false);
/// A stop (a person's, MCP's, the setting switched off) came while cloudflared was downloaded:
/// the start after the download is dropped. Set only under the `STATE` lock while `DOWNLOADING`,
/// read under it where the start would become a presentation.
pub(super) static ABORT: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Whether a presentation runs (starting or live).
#[must_use]
pub fn is_running() -> bool {
    lock(&STATE).is_some()
}

/// `portal.present.start` (`{ "consent"?: bool, "download"?: bool }`): a person's start. The
/// first start needs `consent` (the traffic runs through Cloudflare), a missing cloudflared
/// `download`; without them the Portal panel opens with the question and nothing starts.
///
/// # Errors
///
/// The presentation is off, an argument is no boolean, the listener or the program cannot start.
pub fn start(core: &Arc<Core>, args: &Value) -> Result<Value, CommandError> {
    let consent = flag(args, "consent")?;
    let download = flag(args, "download")?;
    let user = core.user_settings();
    let settings = user.portal.presentation.clone();
    if !settings.enabled {
        core.notify(notice(NotificationLevel::Warning, notice::OFF, &[]));
        return Err(CommandError::Failed(
            "portal.presentation.enabled is off; a person switches it on in Settings > Security \
             & Privacy > Portal and pairing"
                .to_owned(),
        ));
    }
    if is_running() || DOWNLOADING.load(Ordering::Acquire) {
        super::open_panel(core);
        return Ok(json!({ "running": true }));
    }
    if !settings.consented {
        if !consent {
            core.notify(notice(NotificationLevel::Warning, notice::CONSENT, &[]));
            super::open_panel(core);
            return Ok(json!({ "running": false, "needs": "consent" }));
        }
        record_consent(core);
    }
    let custom = user.tools.cloudflared_path.clone();
    if let Some(program) = cloudflared::locate_default(custom.as_deref()) {
        return launch(core, &settings, program);
    }
    let Some(offer) = cloudflared::download() else {
        return Err(CommandError::Failed(
            "no cloudflared is pinned for this platform; set tools.cloudflaredPath".to_owned(),
        ));
    };
    if !download {
        core.notify(notice(
            NotificationLevel::Warning,
            notice::NEEDS_DOWNLOAD,
            &offer.args(),
        ));
        super::open_panel(core);
        return Ok(json!({ "running": false, "needs": "download" }));
    }
    fetch_then_launch(core, &offer);
    Ok(json!({ "running": false, "downloading": true }))
}

fn flag(args: &Value, name: &str) -> Result<bool, CommandError> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(on)) => Ok(*on),
        Some(_) => Err(CommandError::InvalidArgs(format!(
            "{name} is true or false"
        ))),
    }
}

/// The person agreed: `portal.presentation.consented` in their settings file and in effect.
fn record_consent(core: &Core) {
    if let Some(path) = core.user_settings_path() {
        let written = crate::settings_window::user_file::update(path, |root| {
            let portal = root.entry("portal").or_insert_with(|| json!({}));
            if !portal.is_object() {
                *portal = json!({});
            }
            let presentation = &mut portal["presentation"];
            if !presentation.is_object() {
                *presentation = json!({});
            }
            presentation["consented"] = json!(true);
        });
        if let Err(error) = written {
            tracing::warn!(%error, "portal.presentation.consented was not saved");
        }
    }
    let mut user = core.user_settings();
    user.portal.presentation.consented = true;
    core.replace_user_settings(user);
}

/// Downloads cloudflared on a blocking thread, then starts.
fn fetch_then_launch(core: &Arc<Core>, offer: &cloudflared::Download) {
    ABORT.store(false, Ordering::Release);
    DOWNLOADING.store(true, Ordering::Release);
    core.notify(notice(
        NotificationLevel::Info,
        notice::DOWNLOADING,
        &offer.args(),
    ));
    core.send(&CoreToShell::PortalChanged);
    let weak = Arc::downgrade(core);
    core.runtime().spawn_blocking(move || {
        let installed = cloudflared::install_default()
            .map(|installed| installed.executable)
            .map_err(|error| error.to_string());
        let Some(core) = weak.upgrade() else {
            DOWNLOADING.store(false, Ordering::Release);
            return;
        };
        after_download(&core, installed);
    });
}

/// The download ended: the start goes on only if no stop came meanwhile and the presentation is
/// still switched on and consented to (both read again, they may have changed).
pub(super) fn after_download(core: &Arc<Core>, installed: Result<PathBuf, String>) {
    let started = installed.map_err(CommandError::Failed).and_then(|program| {
        let settings = core.user_settings().portal.presentation;
        if ABORT.load(Ordering::Acquire) || !settings.enabled || !settings.consented {
            return Ok(json!({ "running": false }));
        }
        launch(core, &settings, program)
    });
    DOWNLOADING.store(false, Ordering::Release);
    ABORT.store(false, Ordering::Release);
    match started {
        Ok(answer) if answer["running"] == true => {}
        Ok(_) => {
            tracing::info!("the presentation's start was dropped after the download");
            core.send(&CoreToShell::PortalChanged);
        }
        Err(error) => {
            tracing::warn!(%error, "the presentation did not start after the download");
            core.notify(notice(
                NotificationLevel::Error,
                notice::FAILED,
                &[("error", error.to_string())],
            ));
            core.send(&CoreToShell::PortalChanged);
        }
    }
}

/// The listener on loopback, cloudflared to it, the grid followed.
fn launch(
    core: &Arc<Core>,
    settings: &PresentationSettings,
    program: PathBuf,
) -> Result<Value, CommandError> {
    let failed = |error: String| {
        core.notify(notice(
            NotificationLevel::Error,
            notice::FAILED,
            &[("error", error.clone())],
        ));
        CommandError::Failed(error)
    };
    let host = core.host();
    let config = host.config();
    let page = host
        .active_view()
        .and_then(|view| host.view(view))
        .map(|info| info.url);
    let base = base_path(
        config.environments.iter().map(|env| env.url.as_str()),
        page.as_deref(),
    );
    let rules = PathRules::new(&base, &settings.allow_paths, &settings.deny_paths);
    let public = Arc::new(Public::new(rules, settings.pointer));
    let weak = Arc::downgrade(core);
    let server = PortalServer::start_public(
        core.runtime(),
        Options {
            ip: std::net::Ipv4Addr::LOCALHOST,
            port: None,
            pairing_ttl: settings.link_ttl(),
            max_devices: settings.viewer_limit(),
            agent: Arc::from(PORTAL_AGENT_SOURCE),
            on_change: Arc::new(move || {
                if let Some(core) = weak.upgrade() {
                    core.send(&CoreToShell::PortalChanged);
                }
            }),
        },
        public.clone(),
    )
    .map_err(|error| failed(format!("the presentation cannot listen: {error}")))?;
    let spawned = {
        let _entered = core.runtime().enter();
        Tunnel::spawn(&program, server.address().port())
    };
    let (tunnel, events) = match spawned {
        Ok(spawned) => spawned,
        Err(error) => {
            server.stop();
            return Err(failed(format!("cloudflared could not be started: {error}")));
        }
    };
    crate::portal::app::follow_grid(core, &server);
    let generation = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    {
        let mut state = lock(&STATE);
        // A stop during the download, taken after `after_download` looked: nothing goes public.
        if ABORT.swap(false, Ordering::AcqRel) {
            drop(state);
            server.stop();
            core.runtime().spawn(tunnel.stop());
            return Ok(json!({ "running": false }));
        }
        *state = Some(Running {
            server,
            public,
            tunnel: Some(tunnel),
            phase: Phase::Starting,
            link: None,
            started_with: settings.session(),
            started_at: SystemTime::now(),
            generation,
        });
    }
    tracing::info!("the presentation opens a Cloudflare tunnel");
    core.runtime()
        .spawn(supervise(Arc::downgrade(core), generation, events));
    core.notify(notice(NotificationLevel::Info, notice::STARTING, &[]));
    core.send(&CoreToShell::PortalChanged);
    super::open_panel(core);
    Ok(json!({ "running": true }))
}

/// Follows the tunnel of run `generation`: its address, its connection, its end.
async fn supervise(core: Weak<Core>, generation: u64, mut events: UnboundedReceiver<TunnelEvent>) {
    let deadline = tokio::time::sleep(START_TIMEOUT);
    tokio::pin!(deadline);
    let mut live = false;
    loop {
        tokio::select! {
            event = events.recv() => {
                let Some(core) = core.upgrade() else {
                    return;
                };
                match event {
                    Some(TunnelEvent::Url(url)) => {
                        if !on_address(generation, &url) {
                            end(&core, generation, notice::FAILED_ADDRESS);
                            return;
                        }
                    }
                    Some(TunnelEvent::Connected) => {
                        live = true;
                        on_connected(&core, generation);
                    }
                    Some(TunnelEvent::Exited { stopped: true }) => return,
                    Some(TunnelEvent::Exited { stopped: false }) | None => {
                        end(&core, generation, notice::ABORTED);
                        return;
                    }
                }
            }
            () = &mut deadline, if !live => {
                if let Some(core) = core.upgrade() {
                    end(&core, generation, notice::TIMEOUT);
                }
                return;
            }
        }
    }
}

fn on_address(generation: u64, url: &str) -> bool {
    let state = lock(&STATE);
    match state.as_ref() {
        Some(running) if running.generation == generation => running.public.set_address(url),
        _ => true,
    }
}

/// The edge took the tunnel: the presentation is live, with a first link.
fn on_connected(core: &Core, generation: u64) {
    let server = {
        let mut state = lock(&STATE);
        let Some(running) = state
            .as_mut()
            .filter(|running| running.generation == generation)
        else {
            return;
        };
        running.phase = Phase::Live;
        running.server.clone()
    };
    tracing::info!("the presentation is live");
    if let Err(error) = new_link_on(&server) {
        tracing::warn!(%error, "no first presentation link");
    }
    core.notify(notice(NotificationLevel::Warning, notice::STARTED, &[]));
    core.send(&CoreToShell::PortalChanged);
}

/// `portal.present.new_link`: a fresh link (the one before is void) in the Portal panel.
///
/// # Errors
///
/// No presentation is live, or the OS gives no randomness.
pub fn new_link(core: &Core) -> Result<Value, CommandError> {
    let server = lock(&STATE)
        .as_ref()
        .filter(|running| running.phase == Phase::Live)
        .map(|running| running.server.clone())
        .ok_or_else(|| CommandError::Failed("no presentation is live".to_owned()))?;
    new_link_on(&server).map_err(|error| CommandError::Failed(error.to_string()))?;
    super::open_panel(core);
    Ok(json!({ "linkOpen": true }))
}

fn new_link_on(server: &PortalServer) -> std::io::Result<()> {
    let Some(link) = server.issue_link()? else {
        return Ok(());
    };
    if let Some(running) = lock(&STATE).as_mut() {
        running.link = Some((link.url, SystemTime::now() + link.expires_in));
    }
    server.changed();
    Ok(())
}

/// Ends run `generation` (a crash, a timeout, a wrong address) and says why.
fn end(core: &Core, generation: u64, key: &str) {
    let running = {
        let mut state = lock(&STATE);
        if state.as_ref().map(|running| running.generation) != Some(generation) {
            return;
        }
        state.take()
    };
    if let Some(running) = running {
        tracing::warn!(reason = key, "the presentation ended by itself");
        finish(core, running);
        core.notify(notice(NotificationLevel::Error, key, &[]));
        core.send(&CoreToShell::PortalChanged);
    }
}

/// `portal.present.stop`, `portal_present_stop`: every viewer is disconnected, the tunnel
/// closed; during cloudflared's download the start after it is dropped; `false` when neither
/// ran.
pub fn stop(core: &Core) -> bool {
    let running = {
        let mut state = lock(&STATE);
        let running = state.take();
        if running.is_none() && DOWNLOADING.load(Ordering::Acquire) {
            ABORT.store(true, Ordering::Release);
            drop(state);
            tracing::info!("the presentation's start was stopped during the download");
            core.notify(notice(NotificationLevel::Info, notice::STOPPED, &[]));
            core.send(&CoreToShell::PortalChanged);
            return true;
        }
        running
    };
    let Some(running) = running else {
        return false;
    };
    finish(core, running);
    tracing::info!("the presentation was stopped");
    core.notify(notice(NotificationLevel::Info, notice::STOPPED, &[]));
    core.send(&CoreToShell::PortalChanged);
    true
}

/// The listener stops (every viewer's session, socket and forwarded connection ends, the links
/// leave the log mask), cloudflared is killed and waited for.
fn finish(core: &Core, mut running: Running) {
    running.server.stop();
    if let Some((url, _)) = running.link.take() {
        rb_diag::unregister_secret(&url);
    }
    if let Some(tunnel) = running.tunnel.take() {
        core.runtime().spawn(tunnel.stop());
    }
}

/// rBrowser ends: the listener stops, and dropping the tunnel kills cloudflared.
pub fn shutdown() {
    if let Some(mut running) = lock(&STATE).take() {
        running.server.stop();
        drop(running.tunnel.take());
    }
}

/// The user settings changed: switched off or any key but the consent changed ends a running
/// presentation (a lower viewer limit must not wait for the next start); switched off during
/// cloudflared's download drops the start after it.
pub fn on_user_settings_changed(core: &Core) {
    let settings = core.user_settings().portal.presentation;
    let changed = lock(&STATE).as_ref().map_or(
        !settings.enabled && DOWNLOADING.load(Ordering::Acquire),
        |running| !settings.enabled || running.started_with != settings.session(),
    );
    if changed {
        stop(core);
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
