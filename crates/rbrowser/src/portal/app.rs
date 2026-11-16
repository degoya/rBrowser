//! The portal in the app (RB-080-05): starting and stopping it, the open pairing, the panel's
//! state, and the glue to the grid - the active view's page and the leader's events go to the
//! devices. One portal per process. It never starts by itself: `portal.enabled` (a person's
//! setting, protected from MCP) allows it, and a person's `portal.toggle` or `portal.show_qr`
//! starts it; no MCP tool does.

use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rb_config::PortalSettings;
use rb_engine::{ViewEvent, ViewHost as _};
use rb_ipc::{
    CoreToShell, Notification, NotificationLevel, PortalDevice, PortalPairing, PortalState,
    ShellToCore,
};
use rb_sync::{AgentMessage, SyncSwitches};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, watch};

use super::interface::{InterfaceError, listen_address};
use super::origins::allowlist;
use super::qr::QrMatrix;
use super::server::{Options, PortalServer};
use super::state::DeviceStatus;
use super::{ENABLED_KEY, PORTAL_AGENT_SOURCE};
use crate::cdp::lock;
use crate::control::Core;
use crate::host::Host;
use crate::ipc::Failure;
use crate::sync_agent::AgentReport;

/// Why the portal did not start.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    /// `portal.enabled` is off.
    #[error(
        "portal.disabled: the portal is off; a person switches it on in Settings > Security & \
         Privacy > Portal (portal.enabled)"
    )]
    Disabled,
    /// No address to listen on.
    #[error("{0}")]
    Interface(#[from] InterfaceError),
    /// The address or port cannot be taken.
    #[error("the portal cannot listen: {0}")]
    Io(#[from] std::io::Error),
}

/// What [`start`] did.
pub struct Started {
    /// The portal.
    pub server: Arc<PortalServer>,
    /// It was started now (not running already).
    pub fresh: bool,
}

struct Open {
    url: String,
    expires_at: SystemTime,
}

#[derive(Default)]
struct State {
    server: Option<Arc<PortalServer>>,
    /// The `portal` settings it started with: any change restarts nothing, it stops (a lower
    /// `maxDevices` or `pairingTtlSec` must not wait for the next start; Charge 10 review L5).
    started_with: Option<PortalSettings>,
    /// The last pairing URL issued (it holds the token): for the panel only.
    pairing: Option<Open>,
}

static STATE: Mutex<State> = Mutex::new(State {
    server: None,
    started_with: None,
    pairing: None,
});

/// The running portal.
pub fn current() -> Option<Arc<PortalServer>> {
    lock(&STATE).server.clone()
}

/// Starts the portal when `portal.enabled` allows it; the running one when it runs.
///
/// # Errors
///
/// [`StartError`].
pub fn start(core: &Arc<Core>) -> Result<Started, StartError> {
    let settings = core.user_settings().portal;
    if !settings.enabled {
        return Err(StartError::Disabled);
    }
    let mut state = lock(&STATE);
    if let Some(server) = &state.server {
        return Ok(Started {
            server: server.clone(),
            fresh: false,
        });
    }
    let ip = listen_address(&settings.interface)?;
    let weak = Arc::downgrade(core);
    let server = PortalServer::start(
        core.runtime(),
        Options {
            ip,
            port: settings.port,
            pairing_ttl: settings.pairing_ttl(),
            max_devices: settings.device_limit(),
            agent: Arc::from(PORTAL_AGENT_SOURCE),
            on_change: Arc::new(move || {
                if let Some(core) = weak.upgrade() {
                    core.send(&CoreToShell::PortalChanged);
                }
            }),
        },
    )?;
    tracing::info!(address = %server.address(), "the portal listens in the LAN");
    refresh_page(core.host(), &server);
    // Subscribed here, not in the task: a config change right after the start must reach it.
    let feeds = Feeds {
        views: core.host().subscribe(),
        reports: core.host().sync_agents().subscribe(),
        stopped: server.stopped.subscribe(),
    };
    core.runtime().spawn(follow(
        Arc::downgrade(core.host()),
        Arc::downgrade(&server),
        feeds,
    ));
    state.server = Some(server.clone());
    state.started_with = Some(settings.clone());
    drop(state);
    // Every start says it: the page is reachable in the LAN now.
    core.notify(notification(
        NotificationLevel::Warning,
        "portal.notice.started",
        &[("address", server.origin().to_owned())],
    ));
    core.send(&CoreToShell::PortalChanged);
    Ok(Started {
        server,
        fresh: true,
    })
}

/// Stops the portal; `false` when it was not running.
pub fn stop(core: &Core) -> bool {
    let stopped = stop_quietly();
    if stopped {
        core.notify(notification(
            NotificationLevel::Info,
            "portal.notice.stopped",
            &[],
        ));
        core.send(&CoreToShell::PortalChanged);
    }
    stopped
}

fn stop_quietly() -> bool {
    let mut state = lock(&STATE);
    state.pairing = None;
    state.started_with = None;
    match state.server.take() {
        Some(server) => {
            server.stop();
            tracing::info!("the portal stopped");
            true
        }
        None => false,
    }
}

/// The app quits.
pub fn shutdown() {
    stop_quietly();
}

/// The user settings changed: when any `portal.*` key differs from the start (switched off,
/// moved, a lower limit), the portal stops (a person starts it again).
pub fn on_user_settings_changed(core: &Core) {
    let settings = core.user_settings().portal;
    let changed = {
        let state = lock(&STATE);
        state.server.is_some()
            && (!settings.enabled || state.started_with.as_ref() != Some(&settings))
    };
    if changed {
        stop(core);
    }
}

/// A new one-time pairing on the running portal (the one before is void).
///
/// # Errors
///
/// The OS gives no randomness.
pub fn new_pairing(server: &PortalServer) -> std::io::Result<()> {
    let pairing = server.issue_pairing()?;
    lock(&STATE).pairing = Some(Open {
        url: pairing.url,
        expires_at: SystemTime::now() + pairing.expires_in,
    });
    server.changed();
    Ok(())
}

/// The panel's state; `with_pairing` adds the open pairing and its QR code.
#[must_use]
pub fn panel_state(core: &Core, with_pairing: bool) -> PortalState {
    let enabled = core.user_settings().portal.enabled;
    let (server, open) = {
        let state = lock(&STATE);
        let open = state
            .pairing
            .as_ref()
            .map(|open| (open.url.clone(), open.expires_at));
        (state.server.clone(), open)
    };
    let Some(server) = server else {
        return PortalState {
            enabled,
            ..PortalState::default()
        };
    };
    let pairing = open
        .filter(|_| with_pairing && server.pairing_open(Instant::now()))
        .and_then(|(url, expires_at)| {
            let matrix = QrMatrix::encode(&url).ok()?;
            Some(PortalPairing {
                url,
                size: u32::try_from(matrix.size).ok()?,
                rows: matrix.rows,
                expires_at_ms: epoch_ms(expires_at),
            })
        });
    PortalState {
        enabled,
        running: true,
        address: Some(server.origin().to_owned()),
        devices: server.devices().into_iter().map(device).collect(),
        pairing,
    }
}

/// `getPortal`.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    matches!(message, ShellToCore::GetPortal)
        .then(|| Ok(CoreToShell::Portal(Box::new(panel_state(core, true)))))
}

fn device(status: DeviceStatus) -> PortalDevice {
    PortalDevice {
        id: status.info.id,
        name: status.info.name,
        ip: status.info.ip,
        paired_at_ms: status.info.paired_at_ms,
        last_seen_ms: status.info.last_seen_ms,
        contacted: status.info.contacted,
        connected: status.connected,
        replayed: status.counts.replayed,
        failed: status.counts.failed,
        skipped: status.counts.skipped,
    }
}

/// What `follow` listens to, subscribed before the task runs.
struct Feeds {
    views: broadcast::Receiver<ViewEvent>,
    reports: broadcast::Receiver<AgentReport>,
    stopped: watch::Receiver<bool>,
}

/// The devices follow the grid: the active view's page, and its agent's events.
async fn follow(host: Weak<Host>, server: Weak<PortalServer>, feeds: Feeds) {
    let Feeds {
        mut views,
        mut reports,
        mut stopped,
    } = feeds;
    loop {
        tokio::select! {
            event = views.recv() => {
                let (Some(host), Some(server)) = (host.upgrade(), server.upgrade()) else {
                    return;
                };
                match event {
                    Ok(
                        ViewEvent::ActiveChanged(_)
                        | ViewEvent::Navigated { .. }
                        | ViewEvent::Closed(_)
                        | ViewEvent::LayoutChanged
                        | ViewEvent::StateChanged,
                    )
                    | Err(RecvError::Lagged(_)) => refresh_page(&host, &server),
                    Ok(_) => {}
                    Err(RecvError::Closed) => return,
                }
            }
            report = reports.recv() => {
                let report = match report {
                    Ok(report) => report,
                    Err(RecvError::Lagged(missed)) => {
                        tracing::debug!(missed, "portal: leader events dropped");
                        continue;
                    }
                    Err(RecvError::Closed) => return,
                };
                let (Some(host), Some(server)) = (host.upgrade(), server.upgrade()) else {
                    return;
                };
                // The leader's top frame only: a device has no frames of the grid's to match.
                if let AgentMessage::Event(message) = &report.message
                    && message.top
                    && message.frame.is_none()
                    && host.active_view() == Some(report.view)
                {
                    server.broadcast(&message.event, &switches(&host));
                }
            }
            changed = stopped.changed() => {
                if changed.is_err() || *stopped.borrow() {
                    return;
                }
            }
        }
    }
}

/// The active view's page and the project's origins, for the devices.
fn refresh_page(host: &Host, server: &PortalServer) {
    let config = host.config();
    let origins = allowlist(
        config.start_url.as_deref(),
        config.environments.iter().map(|env| env.url.as_str()),
    );
    let url = host
        .active_view()
        .and_then(|view| host.view(view))
        .map(|info| info.url);
    server.set_page(url, origins);
}

/// The sync switches now; the project's when the hub does not run.
fn switches(host: &Host) -> SyncSwitches {
    crate::sync_hub::for_host(host).map_or_else(
        || crate::sync_hub::switches_from(&host.config().sync),
        |sync| sync.hub().with(|hub| hub.switches()),
    )
}

fn notification(level: NotificationLevel, key: &str, args: &[(&str, String)]) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args: args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        settings_key: Some(ENABLED_KEY.to_owned()),
    })
}

fn epoch_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
