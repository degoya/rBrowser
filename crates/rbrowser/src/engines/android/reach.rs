//! Which reverses rBrowser holds (RB-096-12): [`Reverses`] sets `adb reverse` for the
//! loopback port a view loads before it loads ([`Reverses::prepare`]), counts the views that
//! need each port per device, and removes a reverse when the last of them leaves it - it loads
//! another address ([`Reverses::prepare`] again: a view holds only the port of the address it
//! loads now) or closes ([`Reverses::release`]) - or all of them when Android or
//! `engines.android.reverseLocalhost` is switched off and when the app quits
//! ([`Reverses::clear`]). One [`Reverses`] serves the whole process ([`Reverses::global`]), so
//! two windows on one device share a port instead of taking it from each other. Each device has
//! a lock of its own: adb runs under it, never under the lock of the whole map, so a slow device
//! stalls no other. What it sets is noted in the [`Ledger`]; the leftovers of a run that ended
//! without cleaning up are dropped from it at the start, never removed from the device (adb names
//! no owner). [`ReverseLink`] is one view's side: its holder id, and the notice when the setting
//! is off. The calls and the rules are `super::reverse`'s.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use rb_ipc::{CoreToShell, Notification, NotificationLevel};

use super::adb::{AdbError, AdbRun, shown_serial};
use super::devices::valid_serial;
use super::reverse::{
    Decision, Listed, address_of, decide, loopback_port, parse_reverse_list, reverse_args,
    reverse_list_args, unreverse_args,
};
use super::reverse_ledger::Ledger;
use crate::cdp::lock;

/// The notice of a loopback address the device cannot reach; `{address}`.
pub const OFF_KEY: &str = "engines.android.reverseOff";
/// The setting the notice links.
pub const SETTING: &str = "engines.android.reverseLocalhost";

/// Shows a notice in the window of the view.
pub type Notifier = Arc<dyn Fn(CoreToShell) + Send + Sync>;

/// What a view's address needs on the device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// No loopback address: nothing to do.
    Remote,
    /// The port was reversed now.
    Set(u16),
    /// rBrowser's reverse of the port stands already.
    Stands(u16),
    /// Someone else's reverse holds the port; left alone.
    Foreign(u16),
    /// `engines.android.reverseLocalhost` is off: the device does not reach the port.
    Off(u16),
    /// adb did not do it.
    Failed(u16),
}

/// The ports rBrowser reversed on one device, each with the views that need it.
struct Device {
    adb: Arc<dyn AdbRun>,
    ports: BTreeMap<u16, BTreeSet<u64>>,
}

impl Device {
    fn hold(&mut self, port: u16, holder: u64) {
        self.ports.entry(port).or_default().insert(holder);
    }

    /// `holder` leaves every port but `keep`; the ports no view needs any more.
    fn leave(&mut self, holder: u64, keep: Option<u16>) -> Vec<u16> {
        let mut freed = Vec::new();
        for (port, holders) in &mut self.ports {
            if Some(*port) != keep && holders.remove(&holder) && holders.is_empty() {
                freed.push(*port);
            }
        }
        for port in &freed {
            self.ports.remove(port);
        }
        freed
    }
}

type DeviceLock = Arc<tokio::sync::Mutex<Device>>;

/// The one [`Reverses`] of the process, once an Android engine made it.
static GLOBAL: OnceLock<Arc<Reverses>> = OnceLock::new();

/// The reverses rBrowser holds.
pub struct Reverses {
    allowed: AtomicBool,
    ledger: Ledger,
    /// The devices, each behind its own lock; this map's lock is held across no await.
    devices: Mutex<HashMap<String, DeviceLock>>,
}

impl Reverses {
    /// Reverses allowed or not, noted in `ledger`; what the ledger holds of processes that
    /// `alive` says ended is dropped now (never removed from a device).
    pub fn new(
        allowed: bool,
        ledger: Ledger,
        alive: impl Fn(u32) -> bool + Send + Sync + 'static,
    ) -> Arc<Self> {
        ledger.sweep(alive);
        Arc::new(Self {
            allowed: AtomicBool::new(allowed),
            ledger,
            devices: Mutex::new(HashMap::new()),
        })
    }

    /// The process's one [`Reverses`], its ledger in the local folder.
    pub fn global() -> Arc<Self> {
        GLOBAL
            .get_or_init(|| {
                Self::new(
                    true,
                    Ledger::default_place(),
                    crate::extensions::store::process_alive,
                )
            })
            .clone()
    }

    /// The process's [`Reverses`] when an Android engine made it (the app quitting).
    pub fn existing() -> Option<Arc<Self>> {
        GLOBAL.get().cloned()
    }

    /// `engines.android.reverseLocalhost` in effect.
    pub fn set_allowed(&self, allowed: bool) {
        self.allowed.store(allowed, Ordering::Release);
    }

    fn allowed(&self) -> bool {
        self.allowed.load(Ordering::Acquire)
    }

    /// Whether rBrowser holds `port` on `serial` (the tests).
    #[cfg(test)]
    pub fn holds(&self, serial: &str, port: u16) -> bool {
        self.known(serial).is_some_and(|device| {
            device
                .try_lock()
                .is_ok_and(|device| device.ports.contains_key(&port))
        })
    }

    fn known(&self, serial: &str) -> Option<DeviceLock> {
        lock(&self.devices).get(serial).cloned()
    }

    fn device(&self, serial: &str, adb: &Arc<dyn AdbRun>) -> DeviceLock {
        lock(&self.devices)
            .entry(serial.to_owned())
            .or_insert_with(|| {
                Arc::new(tokio::sync::Mutex::new(Device {
                    adb: adb.clone(),
                    ports: BTreeMap::new(),
                }))
            })
            .clone()
    }

    /// Makes the loopback port of `url` reach this machine for view `holder` on `serial`,
    /// before the page loads; `verify` reads the list again for a port held already (a
    /// reload). The view holds only that port: any other it held is left first (removed when
    /// no other view needs it), and an address that is no loopback one leaves them all. A
    /// failure is logged and the page loads anyway.
    pub async fn prepare(
        &self,
        adb: &Arc<dyn AdbRun>,
        serial: &str,
        holder: u64,
        url: &str,
        verify: bool,
    ) -> Reach {
        let wanted = loopback_port(url);
        let Some(port) = wanted.filter(|_| self.allowed() && valid_serial(serial)) else {
            self.release(serial, holder).await;
            return match wanted {
                None => Reach::Remote,
                Some(port) if !self.allowed() => Reach::Off(port),
                Some(port) => Reach::Failed(port),
            };
        };
        let device = self.device(serial, adb);
        let mut device = device.lock().await;
        // `clear` may have run while this waited for the device: the setting is read again.
        let keep = self.allowed().then_some(port);
        let freed = device.leave(holder, keep);
        self.unset_all(&device.adb.clone(), serial, &freed).await;
        if keep.is_none() {
            return Reach::Off(port);
        }
        device.adb = adb.clone();
        let held = device.ports.contains_key(&port);
        if held && !verify {
            device.hold(port, holder);
            return Reach::Stands(port);
        }
        let listed = match list(adb.as_ref(), serial).await {
            Ok(listed) => listed,
            Err(error) => {
                log_failure(serial, port, &error, "adb reverse --list failed");
                return Reach::Failed(port);
            }
        };
        match decide(port, &listed, held) {
            Decision::Stands => {
                device.hold(port, holder);
                Reach::Stands(port)
            }
            Decision::Foreign => {
                if held {
                    // Someone pointed the port elsewhere: it is no longer rBrowser's.
                    device.ports.remove(&port);
                    self.ledger.remove(serial, port);
                }
                let shown = shown_serial(serial);
                tracing::debug!(serial = %shown, port, "another adb reverse holds the port");
                Reach::Foreign(port)
            }
            Decision::Set => match adb.run(&reverse_args(serial, port)).await {
                Ok(_) => {
                    device.hold(port, holder);
                    self.ledger.add(serial, port);
                    tracing::debug!(serial = %shown_serial(serial), port, "adb reverse set");
                    Reach::Set(port)
                }
                Err(error) => {
                    log_failure(serial, port, &error, "adb reverse failed");
                    Reach::Failed(port)
                }
            },
        }
    }

    /// View `holder` on `serial` closed: the ports no other view needs are removed.
    pub async fn release(&self, serial: &str, holder: u64) {
        let Some(device) = self.known(serial) else {
            return;
        };
        let mut device = device.lock().await;
        let freed = device.leave(holder, None);
        self.unset_all(&device.adb.clone(), serial, &freed).await;
    }

    /// Removes every reverse rBrowser holds (Android or the setting off, the app quitting).
    pub async fn clear(&self) {
        let devices: Vec<(String, DeviceLock)> = lock(&self.devices)
            .iter()
            .map(|(serial, device)| (serial.clone(), device.clone()))
            .collect();
        for (serial, device) in devices {
            let mut device = device.lock().await;
            let ports: Vec<u16> = std::mem::take(&mut device.ports).into_keys().collect();
            self.unset_all(&device.adb.clone(), &serial, &ports).await;
        }
    }

    /// Removes rBrowser's reverses of `ports` and their ledger entries. One adb did not remove
    /// stays in the ledger while the process runs and is not tried again: it may stand until
    /// the device reconnects (the debug log says so).
    async fn unset_all(&self, adb: &Arc<dyn AdbRun>, serial: &str, ports: &[u16]) {
        for port in ports {
            if remove(adb.as_ref(), serial, *port).await {
                self.ledger.remove(serial, *port);
            }
        }
    }
}

async fn list(adb: &dyn AdbRun, serial: &str) -> Result<Vec<Listed>, AdbError> {
    let output = adb.run(&reverse_list_args(serial)).await?;
    Ok(parse_reverse_list(&output))
}

/// `reverse --remove` of `port`; `false` when adb did not.
async fn remove(adb: &dyn AdbRun, serial: &str, port: u16) -> bool {
    match adb.run(&unreverse_args(serial, port)).await {
        Ok(_) => {
            tracing::debug!(serial = %shown_serial(serial), port, "adb reverse removed");
            true
        }
        Err(error) => {
            log_failure(serial, port, &error, "adb reverse --remove failed");
            false
        }
    }
}

/// A failure in the debug log, the serial cut in adb's own text too (RB-096-10).
fn log_failure(serial: &str, port: u16, error: &AdbError, what: &str) {
    let shown = shown_serial(serial);
    let text = if serial.is_empty() {
        error.to_string()
    } else {
        error.to_string().replace(serial, &shown)
    };
    tracing::debug!(serial = %shown, port, error = %text, "{what}");
}

/// Where a window's notices go, set once the window's core is there.
#[derive(Clone, Default)]
pub struct Notices(Arc<Mutex<Option<Notifier>>>);

impl Notices {
    /// Sends the notices to `notifier`.
    pub fn set(&self, notifier: Notifier) {
        *lock(&self.0) = Some(notifier);
    }

    fn tell(&self, message: CoreToShell) {
        let notifier = lock(&self.0).clone();
        if let Some(notifier) = notifier {
            notifier(message);
        }
    }
}

/// One view's side of the reverses.
pub struct ReverseLink {
    reverses: Arc<Reverses>,
    notices: Notices,
    holder: u64,
    /// The ports the view told about already (the setting off).
    told: Mutex<HashSet<u16>>,
}

impl ReverseLink {
    /// A view's link, with a holder id of its own.
    pub fn new(reverses: Arc<Reverses>, notices: Notices) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self {
            reverses,
            notices,
            holder: NEXT.fetch_add(1, Ordering::Relaxed),
            told: Mutex::new(HashSet::new()),
        }
    }

    /// [`Reverses::prepare`] for this view; with the setting off the view tells once per port
    /// that the device cannot reach the address, the setting linked.
    pub async fn prepare(
        &self,
        adb: &Arc<dyn AdbRun>,
        serial: &str,
        url: &str,
        verify: bool,
    ) -> Reach {
        let reach = self
            .reverses
            .prepare(adb, serial, self.holder, url, verify)
            .await;
        if let Reach::Off(port) = reach
            && lock(&self.told).insert(port)
            && let Some(address) = address_of(url)
        {
            self.notices.tell(off_notice(&address));
        }
        reach
    }

    /// The view closed.
    pub async fn release(&self, serial: &str) {
        self.reverses.release(serial, self.holder).await;
    }
}

/// The notice that the device cannot reach `address`, the setting linked.
pub fn off_notice(address: &str) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level: NotificationLevel::Warning,
        message_key: OFF_KEY.to_owned(),
        args: [("address".to_owned(), address.to_owned())].into(),
        settings_key: Some(SETTING.to_owned()),
    })
}

#[cfg(test)]
#[path = "reach_tests.rs"]
mod tests;
