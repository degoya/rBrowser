//! [`AndroidEngine`]: Chrome on Android devices on USB for one window (RB-095-02). A view's
//! breakpoint names its device (`android-<serial>`, `super::devices::breakpoint_id`); opening
//! it lists the devices, forwards a free port on 127.0.0.1 to Chrome's DevTools socket, checks
//! that the forward answers nowhere beyond this machine (`super::forward::exposure`), starts
//! Chrome when it does not answer yet, and opens a tab over CDP ([`super::view`]). A view's
//! loopback ports reach this machine over `adb reverse` while it runs ([`super::reach`],
//! `engines.android.reverseLocalhost`, RB-096-12). Without
//! `engines.android.enabled`, without adb or without the device the engine is unavailable and
//! says why - the reason the view's cell shows.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rb_config::{AndroidSettings, EngineKind};
use rb_engine::{Engine, EngineError, EngineHealth, ViewHandle, ViewId, ViewSpec};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

use super::adb::{self, AdbProgram, AdbRun};
use super::adb_tool;
use super::cdp::CdpClient;
use super::devices::{Device, DeviceState, serial_for};
use super::forward::{
    Exposed, answer_whole, browser_socket, endpoint, exposure, http_body, version_request,
};
use super::locate::{Env, Found, Missing, locate, program_name};
use super::reach::{Notices, Notifier, ReverseLink, Reverses};
use super::reverse_ledger::Ledger;
use super::view::{AndroidView, Link};
use crate::cdp::lock;
use crate::tools::{ToolStore, adb_entry};

/// How often Chrome is asked for its socket after it was started.
const CHROME_TRIES: u32 = 10;
/// The wait between two asks.
const CHROME_WAIT: Duration = Duration::from_millis(500);
/// The most bytes of `/json/version` read.
const MAX_VERSION: u64 = 64 * 1024;
/// How long `/json/version` may take.
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// Makes the adb runner of a found program (the tests put a fake in).
pub type AdbFactory = Arc<dyn Fn(&Found) -> Arc<dyn AdbRun> + Send + Sync>;

/// The Android engine of one window.
pub struct AndroidEngine {
    settings: Mutex<AndroidSettings>,
    store: Option<ToolStore>,
    env: Env,
    factory: AdbFactory,
    /// The machine's own addresses the exposure check asks the forward at.
    own_addresses: fn() -> Vec<IpAddr>,
    views: Mutex<HashMap<ViewId, Arc<AndroidView>>>,
    /// The reverses of the views' loopback ports.
    reverses: Arc<Reverses>,
    /// The window's notices (a loopback address the device cannot reach).
    notices: Notices,
}

impl AndroidEngine {
    /// The engine with `settings`, the managed tools of `store` and this process's variables.
    pub fn new(settings: AndroidSettings, store: Option<ToolStore>) -> Arc<Self> {
        let factory: AdbFactory =
            Arc::new(|found: &Found| Arc::new(AdbProgram::new(found.path.clone())) as _);
        let reverses = Reverses::global();
        reverses.set_allowed(reverses_on(&settings));
        Self::build(
            settings,
            store,
            Env::from_process(),
            factory,
            adb::own_addresses,
            reverses,
        )
    }

    /// [`AndroidEngine::new`] over `env`, `factory` and the source of the machine's own
    /// addresses.
    pub fn with(
        settings: AndroidSettings,
        store: Option<ToolStore>,
        env: Env,
        factory: AdbFactory,
        own_addresses: fn() -> Vec<IpAddr>,
    ) -> Arc<Self> {
        // Reverses of its own, noted nowhere: the tests' engines share nothing.
        let reverses = Reverses::new(reverses_on(&settings), Ledger::at(None), |_| true);
        Self::build(settings, store, env, factory, own_addresses, reverses)
    }

    fn build(
        settings: AndroidSettings,
        store: Option<ToolStore>,
        env: Env,
        factory: AdbFactory,
        own_addresses: fn() -> Vec<IpAddr>,
        reverses: Arc<Reverses>,
    ) -> Arc<Self> {
        Arc::new(Self {
            settings: Mutex::new(settings),
            store,
            env,
            factory,
            own_addresses,
            views: Mutex::new(HashMap::new()),
            reverses,
            notices: Notices::default(),
        })
    }

    /// `engines.android.*` in effect.
    pub fn set_settings(&self, settings: AndroidSettings) {
        self.reverses.set_allowed(reverses_on(&settings));
        *lock(&self.settings) = settings;
    }

    /// Whether the views' loopback ports may be reversed: Android and
    /// `engines.android.reverseLocalhost` on.
    pub fn reverses_wanted(&self) -> bool {
        reverses_on(&lock(&self.settings))
    }

    /// Removes every reverse rBrowser set (Android or the setting switched off).
    pub async fn clear_reverses(&self) {
        self.reverses.clear().await;
    }

    /// Where the views' notices go.
    pub fn set_notifier(&self, notifier: Notifier) {
        self.notices.set(notifier);
    }

    /// Whether `engines.android.enabled` is on.
    pub fn is_enabled(&self) -> bool {
        lock(&self.settings).enabled
    }

    /// The adb that runs: the setting, an installed one, then the downloaded one.
    ///
    /// # Errors
    ///
    /// [`Missing`] when none is there.
    pub fn locate_adb(&self) -> Result<Found, Missing> {
        let setting = lock(&self.settings).adb_executable().map(ToOwned::to_owned);
        let managed = adb_entry()
            .ok()
            .and_then(|entry| adb_tool::managed(self.store.as_ref(), &entry));
        locate(
            setting.as_deref(),
            &self.env,
            managed.as_deref(),
            program_name(cfg!(windows)),
            Path::is_file,
        )
    }

    /// adb to run, or why there is none (the text for the person).
    ///
    /// # Errors
    ///
    /// Android is off or no adb is there.
    pub fn adb(&self) -> Result<Arc<dyn AdbRun>, String> {
        if !self.is_enabled() {
            return Err(rb_i18n::t("engines.android.disabled"));
        }
        let found = self.locate_adb().map_err(|missing| no_adb(&missing))?;
        Ok((self.factory)(&found))
    }

    /// The view `id`, while it is open.
    pub fn view(&self, id: ViewId) -> Option<Arc<AndroidView>> {
        lock(&self.views).get(&id).cloned()
    }

    async fn open(&self, spec: &ViewSpec) -> Result<Arc<AndroidView>, EngineError> {
        let unavailable = |reason: String| EngineError::Unavailable {
            engine: EngineKind::Android,
            reason,
        };
        let id = spec.view.ok_or_else(|| {
            EngineError::Invalid("an Android view needs its cell's id".to_owned())
        })?;
        let adb = self.adb().map_err(unavailable)?;
        let devices = adb::devices(adb.as_ref())
            .await
            .map_err(|error| unavailable(error.to_string()))?;
        let device = ready_device(&spec.breakpoint.id, &devices).map_err(unavailable)?;
        let serial = device.serial.clone();
        let port = adb::forward(adb.as_ref(), &serial)
            .await
            .map_err(|error| unavailable(error.to_string()))?;
        let opened = self
            .connect(id, &serial, port, adb.clone(), &spec.url)
            .await;
        if opened.is_err()
            && let Err(error) = adb::unforward(adb.as_ref(), &serial, port).await
        {
            tracing::debug!(%error, "the adb forward stays until adb ends");
        }
        opened
    }

    async fn connect(
        &self,
        id: ViewId,
        serial: &str,
        port: u16,
        adb: Arc<dyn AdbRun>,
        url: &str,
    ) -> Result<Arc<AndroidView>, EngineError> {
        let unavailable = |reason: String| EngineError::Unavailable {
            engine: EngineKind::Android,
            reason,
        };
        let own_addresses = self.own_addresses;
        let exposed =
            tokio::task::spawn_blocking(move || exposure(port, &own_addresses(), adb::answers))
                .await
                .map_err(|_| unavailable("the forward could not be checked".to_owned()))?;
        match exposed {
            Ok(()) => {}
            Err(Exposed::At(at)) => {
                let address = at.to_string();
                return Err(unavailable(rb_i18n::t_with(
                    "engines.android.exposed",
                    &[("address", &address)],
                )));
            }
            Err(Exposed::Unchecked) => {
                return Err(unavailable(rb_i18n::t("engines.android.unchecked")));
            }
        }
        let socket = chrome_socket(adb.as_ref(), serial, port)
            .await
            .ok_or_else(|| {
                unavailable(rb_i18n::t_with(
                    "engines.android.noChrome",
                    &[("serial", serial)],
                ))
            })?;
        let client = CdpClient::connect(&socket).await?;
        let link = Link {
            serial: serial.to_owned(),
            port,
            adb,
            client,
            reach: ReverseLink::new(self.reverses.clone(), self.notices.clone()),
        };
        AndroidView::open(id, link, url).await
    }
}

/// Whether `settings` let a view's loopback port be reversed: Android and the setting on.
fn reverses_on(settings: &AndroidSettings) -> bool {
    settings.enabled && settings.reverse_localhost
}

/// The device of breakpoint `id` among `devices`, ready for a view; else why not.
///
/// # Errors
///
/// The reason for the person: not connected, not authorised, another state.
pub fn ready_device<'a>(id: &str, devices: &'a [Device]) -> Result<&'a Device, String> {
    let serial = id
        .strip_prefix(super::devices::BREAKPOINT_PREFIX)
        .unwrap_or(id);
    let device = serial_for(id, devices)
        .ok_or_else(|| rb_i18n::t_with("engines.android.notConnected", &[("serial", serial)]))?;
    match &device.state {
        DeviceState::Device => Ok(device),
        DeviceState::Unauthorized => Err(rb_i18n::t_with(
            "engines.android.unauthorized",
            &[("serial", &device.serial)],
        )),
        DeviceState::NoPermissions => Err(rb_i18n::t_with(
            "engines.android.noPermissions",
            &[("serial", &device.serial)],
        )),
        DeviceState::Offline => Err(rb_i18n::t_with(
            "engines.android.state",
            &[("serial", &device.serial), ("state", "offline")],
        )),
        DeviceState::Other(state) => Err(rb_i18n::t_with(
            "engines.android.state",
            &[("serial", &device.serial), ("state", state)],
        )),
    }
}

/// Why no adb runs, for the person: the setting, or where it was looked for.
pub fn no_adb(missing: &Missing) -> String {
    match missing {
        Missing::SettingNotAFile(path) => rb_i18n::t_with(
            "engines.android.adbPathNoFile",
            &[("path", &path.display().to_string())],
        ),
        Missing::NotFound => rb_i18n::t("engines.android.noAdb"),
    }
}

/// Chrome's browser socket over `port`; Chrome is started once when it does not answer.
async fn chrome_socket(adb: &dyn AdbRun, serial: &str, port: u16) -> Option<String> {
    if let Some(socket) = version(port).await {
        return Some(socket);
    }
    if let Err(error) = adb::start_chrome(adb, serial).await {
        tracing::debug!(%error, "Chrome did not start on the device");
        return None;
    }
    for _ in 0..CHROME_TRIES {
        tokio::time::sleep(CHROME_WAIT).await;
        if let Some(socket) = version(port).await {
            return Some(socket);
        }
    }
    None
}

/// `/json/version` over the forward, read to its browser socket.
async fn version(port: u16) -> Option<String> {
    let ask = async {
        let mut stream = TcpStream::connect(endpoint(port)).await.ok()?;
        stream
            .write_all(version_request(port).as_bytes())
            .await
            .ok()?;
        // Read up to the answer's length: Chrome keeps the connection open after it.
        let mut answer = Vec::new();
        let mut chunk = [0u8; 4096];
        while !answer_whole(&answer) && (answer.len() as u64) < MAX_VERSION {
            match stream.read(&mut chunk).await.ok()? {
                0 => break,
                read => answer.extend_from_slice(&chunk[..read]),
            }
        }
        let answer = String::from_utf8_lossy(&answer).into_owned();
        browser_socket(port, http_body(&answer)?)
    };
    tokio::time::timeout(VERSION_TIMEOUT, ask)
        .await
        .ok()
        .flatten()
}

#[async_trait]
impl Engine for AndroidEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Android
    }

    fn external(&self) -> bool {
        true
    }

    fn health(&self) -> EngineHealth {
        if !self.is_enabled() {
            return EngineHealth::Unavailable(rb_i18n::t("engines.android.disabled"));
        }
        if let Err(missing) = self.locate_adb() {
            return EngineHealth::Unavailable(no_adb(&missing));
        }
        if lock(&self.views).is_empty() {
            EngineHealth::Stopped
        } else {
            EngineHealth::Running
        }
    }

    async fn create_view(&self, spec: &ViewSpec) -> Result<Arc<dyn ViewHandle>, EngineError> {
        let view = self.open(spec).await?;
        lock(&self.views).insert(view.id_of(), view.clone());
        Ok(view)
    }

    /// Closes every view; each removes its forward and its reverses (RB-096-12).
    async fn shutdown(&self) {
        let views: Vec<Arc<AndroidView>> = lock(&self.views).drain().map(|(_, v)| v).collect();
        for view in views {
            if let Err(error) = ViewHandle::close(view.as_ref()).await {
                tracing::debug!(%error, "an Android view did not close");
            }
        }
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
