//! `safaridriver` as a child process (RB-095-03): the driver of the chosen edition (Safari, or
//! Safari Technology Preview with its own driver), started with `--port` on a free port of
//! `127.0.0.1`, without a console window, killed with its handle, and ready once `GET /status`
//! says so. One driver serves every Safari view; when it ends, the engine ends their views as
//! crashed and the external windows' restart (RB-080-38) starts it again. [`Launch`] lets tests
//! hand the engine a fake driver ([`Attached`]) in place of the process.

use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use rb_process::NoConsoleWindow;
use serde_json::{Value, json};
use tokio::process::{Child, Command};

use crate::client::DriverClient;
use crate::error::SafariError;

/// How long the driver may take to answer `GET /status`.
pub const START_TIMEOUT: Duration = Duration::from_secs(15);
/// How often the start asks.
const POLL: Duration = Duration::from_millis(100);
/// How long a killed driver may take to end before the stop gives up waiting.
const STOP_GRACE: Duration = Duration::from_secs(2);

/// Which Safari a view runs in (`engines.safari.technologyPreview`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Edition {
    /// Safari itself, with `/usr/bin/safaridriver`.
    #[default]
    Safari,
    /// Safari Technology Preview, with the driver in its bundle.
    TechnologyPreview,
}

impl Edition {
    /// The edition of the setting `engines.safari.technologyPreview`.
    pub fn from_setting(technology_preview: bool) -> Self {
        if technology_preview {
            Self::TechnologyPreview
        } else {
            Self::Safari
        }
    }

    /// The app bundle.
    pub fn bundle(self) -> &'static Path {
        Path::new(match self {
            Self::Safari => "/Applications/Safari.app",
            Self::TechnologyPreview => "/Applications/Safari Technology Preview.app",
        })
    }

    /// The edition's `safaridriver`.
    pub fn driver(self) -> &'static Path {
        Path::new(match self {
            Self::Safari => "/usr/bin/safaridriver",
            Self::TechnologyPreview => {
                "/Applications/Safari Technology Preview.app/Contents/MacOS/safaridriver"
            }
        })
    }

    /// The `browserName` a new session asks for.
    pub fn browser_name(self) -> &'static str {
        match self {
            Self::Safari => "safari",
            Self::TechnologyPreview => "Safari Technology Preview",
        }
    }
}

/// The capabilities of a new session in `edition`.
pub fn capabilities(edition: Edition) -> Value {
    json!({ "browserName": edition.browser_name() })
}

/// The driver's command line: the port it listens on (on `localhost` only).
pub fn arguments(port: u16) -> Vec<String> {
    vec!["--port".to_owned(), port.to_string()]
}

/// A free port on `127.0.0.1`, let go for the driver to take a moment later.
pub fn free_port() -> Result<u16, SafariError> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|error| {
            SafariError::Launch(format!("no free port on 127.0.0.1: {}", error.kind()))
        })
}

/// A `safaridriver` rBrowser started: killed when dropped.
#[derive(Debug)]
pub struct DriverProcess {
    child: Child,
}

impl DriverProcess {
    /// Its process id while it runs.
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Waits until the driver ends by itself.
    pub async fn exited(&mut self) {
        if let Err(error) = self.child.wait().await {
            tracing::debug!(%error, "safaridriver's end could not be awaited");
        }
    }

    /// Ends the driver: a kill (the engine closed its sessions before), then up to
    /// [`STOP_GRACE`] for its end. `safaridriver` keeps no state a kill could lose, so it gets
    /// no request to end by itself first.
    pub async fn stop(mut self) {
        if self.child.try_wait().ok().flatten().is_some() {
            return;
        }
        if let Err(error) = self.child.start_kill() {
            tracing::debug!(%error, "safaridriver could not be stopped");
        }
        if tokio::time::timeout(STOP_GRACE, self.child.wait())
            .await
            .is_err()
        {
            tracing::debug!("safaridriver did not end in time");
        }
    }
}

/// A driver to talk to: its client and, when rBrowser started it, its process.
#[derive(Debug)]
pub struct Launched {
    /// The driver's client.
    pub client: DriverClient,
    /// The process rBrowser started; `None` for a driver started elsewhere.
    pub process: Option<DriverProcess>,
}

/// How the engine gets its driver.
#[async_trait]
pub trait Launch: Send + Sync {
    /// Starts (or finds) the driver of `edition`.
    async fn launch(&self, edition: Edition) -> Result<Launched, SafariError>;
}

/// The edition's `safaridriver` on this Mac.
#[derive(Clone, Copy, Debug, Default)]
pub struct DriverLaunch;

#[async_trait]
impl Launch for DriverLaunch {
    async fn launch(&self, edition: Edition) -> Result<Launched, SafariError> {
        let driver = edition.driver();
        if !driver.is_file() {
            let path = driver.display().to_string();
            return Err(SafariError::Launch(rb_i18n::t_with(
                "engines.safari.notFound",
                &[("path", path.as_str())],
            )));
        }
        start(driver, START_TIMEOUT).await
    }
}

/// A driver that runs already on `port` (the tests' fake).
#[derive(Clone, Copy, Debug)]
pub struct Attached(pub u16);

#[async_trait]
impl Launch for Attached {
    async fn launch(&self, _edition: Edition) -> Result<Launched, SafariError> {
        Ok(Launched {
            client: DriverClient::new(self.0),
            process: None,
        })
    }
}

/// Starts `executable` on a free port and waits up to `timeout` until it is ready.
pub async fn start(executable: &Path, timeout: Duration) -> Result<Launched, SafariError> {
    let port = free_port()?;
    let mut command = Command::new(executable);
    command
        .args(arguments(port))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .no_console_window();
    let mut child = command.spawn().map_err(|error| {
        SafariError::Launch(format!(
            "safaridriver could not be started from {}: {}",
            executable.display(),
            error.kind()
        ))
    })?;
    let client = DriverClient::new(port);
    let ready = tokio::time::timeout(timeout, async {
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                return Err(SafariError::Launch(format!(
                    "safaridriver ended at its start ({status})"
                )));
            }
            if client.ready().await.unwrap_or(false) {
                return Ok(());
            }
            tokio::time::sleep(POLL).await;
        }
    })
    .await;
    match ready {
        Ok(Ok(())) => Ok(Launched {
            client,
            process: Some(DriverProcess { child }),
        }),
        Ok(Err(error)) => Err(error),
        Err(_) => {
            if let Err(error) = child.start_kill() {
                tracing::debug!(%error, "safaridriver could not be stopped");
            }
            Err(SafariError::Launch(format!(
                "safaridriver did not answer within {} s",
                timeout.as_secs()
            )))
        }
    }
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
