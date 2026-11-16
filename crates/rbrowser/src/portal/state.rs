//! What a running portal knows (RB-080-05): the pairings, each device's open sockets and
//! replay counts, the active view's page and the project's origins. The app reaches it through
//! the methods on [`PortalServer`] here; the listener through [`Shared`].

use std::collections::HashMap;
use std::future::Future;
use std::time::{Duration, Instant};

use rb_sync::{SyncEvent, SyncSwitches};
use serde::Serialize;
use tokio::sync::{mpsc, watch};

use super::pairing::{DeviceId, DeviceInfo, Pairings, Secret};
use super::relay::{Counts, Report, Skip, ToDevice, for_device, navigate};
use super::server::{PAIR_PATH, PortalServer};
use crate::cdp::lock;

/// Messages queued per socket; a device that falls this far behind loses the newest.
pub const SOCKET_QUEUE: usize = 256;
/// Agent sockets one device may hold at once (one per open page of the portal).
pub const MAX_DEVICE_SOCKETS: usize = 8;

/// One open socket of a device.
pub(super) struct Socket {
    pub(super) id: u64,
    pub(super) sender: mpsc::Sender<String>,
}

/// The portal's state behind its lock.
pub struct Shared {
    pub(super) pairings: Pairings,
    pub(super) sockets: HashMap<DeviceId, Vec<Socket>>,
    pub(super) counts: HashMap<DeviceId, Counts>,
    pub(super) current_url: Option<String>,
    pub(super) allowlist: Vec<String>,
    next_socket: u64,
    /// Per device: `true` once it is disconnected; its forwarded connections watch it.
    revocations: HashMap<DeviceId, watch::Sender<bool>>,
    /// The pairing token last issued, as registered with the log mask.
    issued: Option<Secret>,
}

impl Shared {
    /// What `device`'s forwarded connections watch: it turns `true` (or closes) when the device
    /// is disconnected or the portal stops.
    pub(super) fn watch_device(&mut self, device: DeviceId) -> watch::Receiver<bool> {
        self.revocations
            .entry(device)
            .or_insert_with(|| watch::channel(false).0)
            .subscribe()
    }
}

impl Shared {
    /// No device, no page.
    #[must_use]
    pub fn new(pairing_ttl: Duration, max_devices: usize) -> Self {
        Self {
            pairings: Pairings::new(pairing_ttl, max_devices),
            sockets: HashMap::new(),
            counts: HashMap::new(),
            current_url: None,
            allowlist: Vec::new(),
            next_socket: 1,
            revocations: HashMap::new(),
            issued: None,
        }
    }
}

/// A device in the panel and in `portal_devices`: nothing secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    /// Id, name, address, times.
    #[serde(flatten)]
    pub info: DeviceInfo,
    /// Whether its agent is connected now.
    pub connected: bool,
    /// Its replay counts.
    pub counts: Counts,
}

/// A fresh pairing: the URL for the QR code (it holds the token: the panel only) and how long
/// it lives.
pub struct Pairing {
    /// `http://ip:port/__rbrowser/pair?t=<token>`.
    pub url: String,
    /// Its life from now.
    pub expires_in: Duration,
}

impl std::fmt::Debug for Pairing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Pairing")
            .field("expires_in", &self.expires_in)
            .finish_non_exhaustive()
    }
}

impl PortalServer {
    /// A new one-time pairing token (the one before is void).
    ///
    /// # Errors
    ///
    /// The OS gives no randomness.
    pub fn issue_pairing(&self) -> std::io::Result<Pairing> {
        let now = Instant::now();
        let mut shared = lock(&self.shared);
        let token = shared.pairings.issue(now)?;
        rb_diag::register_secret(token.expose());
        // The one before is void: only the newest token stays in the log mask.
        if let Some(old) = shared.issued.replace(token.clone()) {
            rb_diag::unregister_secret(old.expose());
        }
        let expires_in = shared
            .pairings
            .token_expires()
            .map_or(Duration::ZERO, |at| at.saturating_duration_since(now));
        Ok(Pairing {
            url: format!("{}{PAIR_PATH}?t={}", self.origin(), token.expose()),
            expires_in,
        })
    }

    /// Whether a pairing token is open at `now` (issued, not used, not expired).
    #[must_use]
    pub fn pairing_open(&self, now: Instant) -> bool {
        lock(&self.shared)
            .pairings
            .token_expires()
            .is_some_and(|expires| now < expires)
    }

    /// The active view shows `url` and the project's origins are `allowlist`: the devices
    /// follow when the page changed.
    pub fn set_page(&self, url: Option<String>, allowlist: Vec<String>) {
        let message = {
            let mut shared = lock(&self.shared);
            shared.allowlist = allowlist;
            if shared.current_url == url {
                return;
            }
            shared.current_url.clone_from(&url);
            url.as_deref().and_then(navigate)
        };
        if let Some(message) = message {
            self.send_all(&message);
        }
    }

    /// The leader's `event` for every device, when it can be replayed there.
    pub fn broadcast(&self, event: &SyncEvent, switches: &SyncSwitches) {
        match for_device(event, switches) {
            Ok(message) => self.send_all(&message),
            Err(Skip::Off) => {}
            Err(skip) => {
                let mut shared = lock(&self.shared);
                let connected: Vec<DeviceId> = shared.sockets.keys().copied().collect();
                for device in connected {
                    shared.counts.entry(device).or_default().skipped(skip);
                }
            }
        }
    }

    /// The paired devices, oldest first.
    #[must_use]
    pub fn devices(&self) -> Vec<DeviceStatus> {
        let mut shared = lock(&self.shared);
        for session in shared.pairings.expire_unfinished(Instant::now()) {
            rb_diag::unregister_secret(session.expose());
        }
        shared
            .pairings
            .devices()
            .into_iter()
            .map(|info| DeviceStatus {
                connected: shared
                    .sockets
                    .get(&info.id)
                    .is_some_and(|sockets| !sockets.is_empty()),
                counts: shared.counts.get(&info.id).copied().unwrap_or_default(),
                info,
            })
            .collect()
    }

    /// Revokes `device`'s cookie and closes its sockets and forwarded connections; `false` when
    /// there is no such device.
    pub fn disconnect(&self, device: DeviceId) -> bool {
        let session = {
            let mut shared = lock(&self.shared);
            let session = shared.pairings.disconnect(device);
            // Dropping the senders ends the sockets' tasks, which close them.
            shared.sockets.remove(&device);
            shared.counts.remove(&device);
            if let Some(revocation) = shared.revocations.remove(&device) {
                revocation.send_replace(true);
            }
            session
        };
        let found = session.is_some();
        if let Some(session) = session {
            rb_diag::unregister_secret(session.expose());
            tracing::info!(device, "a portal device was disconnected");
            self.changed();
        }
        found
    }

    /// The portal stops: every session and the token are revoked and leave the log mask, every
    /// forwarded connection ends.
    pub(super) fn forget_all(&self) {
        let (sessions, issued, revocations) = {
            let mut shared = lock(&self.shared);
            shared.sockets.clear();
            (
                shared.pairings.revoke_all(),
                shared.issued.take(),
                std::mem::take(&mut shared.revocations),
            )
        };
        for secret in sessions.iter().chain(issued.iter()) {
            rb_diag::unregister_secret(secret.expose());
        }
        for revocation in revocations.into_values() {
            revocation.send_replace(true);
        }
    }

    /// Ends when `revoked` (from [`Shared::watch_device`]) turns or the portal stops: what a
    /// forwarded connection runs against.
    pub(super) fn ended(
        &self,
        mut revoked: watch::Receiver<bool>,
    ) -> impl Future<Output = ()> + Send + 'static {
        let mut stopped = self.stopped.subscribe();
        async move {
            tokio::select! {
                result = stopped.wait_for(|stopped| *stopped) => drop(result),
                result = revoked.wait_for(|revoked| *revoked) => drop(result),
            }
        }
    }

    /// A new socket of `device`: its id and the messages for it. The device first goes where
    /// the active view is.
    pub(super) fn open_socket(&self, device: DeviceId) -> Option<(u64, mpsc::Receiver<String>)> {
        let (sender, receiver) = mpsc::channel(SOCKET_QUEUE);
        {
            let mut shared = lock(&self.shared);
            let open = shared.sockets.get(&device).map_or(0, Vec::len);
            if !shared.pairings.is_paired(device) || open >= MAX_DEVICE_SOCKETS {
                return None;
            }
            let id = shared.next_socket;
            shared.next_socket += 1;
            if let Some(text) = shared
                .current_url
                .as_deref()
                .and_then(navigate)
                .and_then(|message| serde_json::to_string(&message).ok())
            {
                let _ = sender.try_send(text);
            }
            shared
                .sockets
                .entry(device)
                .or_default()
                .push(Socket { id, sender });
            drop(shared);
            self.changed();
            Some((id, receiver))
        }
    }

    /// `device`'s socket `id` closed.
    pub(super) fn close_socket(&self, device: DeviceId, id: u64) {
        {
            let mut shared = lock(&self.shared);
            if let Some(sockets) = shared.sockets.get_mut(&device) {
                sockets.retain(|socket| socket.id != id);
                if sockets.is_empty() {
                    shared.sockets.remove(&device);
                }
            }
        }
        self.changed();
    }

    /// `device` reported `report`.
    pub(super) fn note(&self, device: DeviceId, report: &Report) {
        lock(&self.shared)
            .counts
            .entry(device)
            .or_default()
            .note(report);
    }

    fn send_all(&self, message: &ToDevice) {
        let Ok(text) = serde_json::to_string(message) else {
            return;
        };
        let shared = lock(&self.shared);
        for socket in shared.sockets.values().flatten() {
            // A full queue drops this message for that device; it stays connected.
            let _ = socket.sender.try_send(text.clone());
        }
    }
}
