//! Starts the real sidecar: the runtime [`crate::node`] finds, checked against `.nvmrc`, running
//! `sidecar/dist/main.js` with stdio piped, without a console window, with an environment from
//! an allowlist ([`ALLOWED_ENV`]) plus `PLAYWRIGHT_BROWSERS_PATH` and, from the user setting
//! `engines.lighthouseBrowserPath` only, `CHROME_PATH` ([`sidecar_env`]): `NODE_OPTIONS`,
//! `NODE_PATH`, `NODE_TLS_REJECT_UNAUTHORIZED`, `NODE_EXTRA_CA_CERTS`, `PLAYWRIGHT_DOWNLOAD_HOST`
//! and the proxy variables could load code into Node, turn off TLS checks or redirect the browser
//! download. Its stderr goes into `tracing` line by line, secrets masked. On Windows it runs in
//! a job object of its own ([`ProcessJob`], RB-080-67), so nothing it starts outlives it. [`ConfiguredLauncher`]
//! reads the settings in effect at each start, so a changed `engines.nodePath` or
//! `engines.playwrightBrowsersPath` applies to the next start (`sidecar.restart`).

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{PoisonError, RwLock};

use rb_process::{NoConsoleWindow, ProcessJob, spawn_in_job};
use tokio::io::{AsyncRead, BufReader};
use tokio::process::Command;

use crate::LOG_TARGET;
use crate::error::SidecarError;
use crate::framing::{LineRead, read_line};
use crate::node::{resolve_node, sidecar_script, verify};
use crate::playwright::browsers_path;
use crate::settings::{SidecarSettings, non_empty};
use crate::supervisor::{Launched, Launcher, Sidecar};

/// The variables of rBrowser's environment Node gets, compared without case (Windows names are
/// case-insensitive). Everything else is left out.
pub const ALLOWED_ENV: &[&str] = &[
    "PATH",
    "SystemRoot",
    "TEMP",
    "TMP",
    "LOCALAPPDATA",
    "USERPROFILE",
    "HOME",
    // Where Lighthouse looks for an installed Chrome or Edge (RB-080-58).
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramW6432",
    "SystemDrive",
];

/// The variables of `vars` in [`ALLOWED_ENV`], in their order.
pub fn allowed_env(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, OsString)> {
    vars.into_iter()
        .filter(|(name, _)| {
            name.to_str().is_some_and(|name| {
                ALLOWED_ENV
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(name))
            })
        })
        .collect()
}

/// The browser of `engines.lighthouseBrowserPath`: an absolute path, else none (the sidecar then
/// finds one itself; a relative path is named in the log).
pub fn lighthouse_browser(setting: Option<&str>) -> Option<PathBuf> {
    let path = PathBuf::from(non_empty(setting)?);
    if path.is_absolute() {
        return Some(path);
    }
    tracing::warn!(
        target: LOG_TARGET,
        path = %path.display(),
        "engines.lighthouseBrowserPath is not an absolute path and is ignored"
    );
    None
}

/// The whole environment of the sidecar: `parent`'s allowed variables, the Playwright browsers
/// folder and - only from the setting, never from `parent` - Lighthouse's `CHROME_PATH`.
pub fn sidecar_env(
    parent: impl IntoIterator<Item = (OsString, OsString)>,
    browsers_path: &Path,
    lighthouse_browser: Option<&Path>,
) -> Vec<(OsString, OsString)> {
    let mut env = allowed_env(parent);
    env.push(("PLAYWRIGHT_BROWSERS_PATH".into(), browsers_path.into()));
    if let Some(browser) = lighthouse_browser {
        env.push(("CHROME_PATH".into(), browser.into()));
    }
    env
}

/// The longest stderr line logged whole.
const MAX_STDERR_LINE: usize = 64 * 1024;

/// Where the package lives and what the settings say, for one start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeLauncher {
    /// The folder of `rbrowser.exe` (`node\`, `sidecar\` beside it).
    pub install_dir: PathBuf,
    /// `engines.nodePath`.
    pub node_path: Option<String>,
    /// The resolved Playwright browsers folder ([`crate::playwright::browsers_path`]).
    pub browsers_path: PathBuf,
    /// `engines.lighthouseBrowserPath` ([`lighthouse_browser`]).
    pub lighthouse_browser: Option<PathBuf>,
}

/// Starts the real sidecar with the settings in effect at each start.
#[derive(Debug)]
pub struct ConfiguredLauncher {
    install_dir: PathBuf,
    local_app_data: Option<OsString>,
    settings: RwLock<SidecarSettings>,
}

impl ConfiguredLauncher {
    /// A launcher for the package in `install_dir`; `local_app_data` is `%LOCALAPPDATA%`.
    pub fn new(
        install_dir: PathBuf,
        local_app_data: Option<OsString>,
        settings: SidecarSettings,
    ) -> Self {
        Self {
            install_dir,
            local_app_data,
            settings: RwLock::new(settings),
        }
    }

    /// The launcher of the package `rbrowser.exe` belongs to.
    pub fn beside_executable(settings: SidecarSettings) -> io::Result<Self> {
        let exe = std::env::current_exe()?;
        let install_dir = exe
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| io::Error::other("the executable has no folder"))?;
        Ok(Self::new(
            install_dir,
            std::env::var_os("LOCALAPPDATA"),
            settings,
        ))
    }

    /// Puts changed settings in effect for the next start.
    pub fn set_settings(&self, settings: SidecarSettings) {
        *self
            .settings
            .write()
            .unwrap_or_else(PoisonError::into_inner) = settings;
    }

    /// The settings in effect.
    pub fn settings(&self) -> SidecarSettings {
        self.settings
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The launcher the settings in effect describe.
    pub fn node_launcher(&self) -> Result<NodeLauncher, SidecarError> {
        let settings = self.settings();
        let browsers = browsers_path(
            settings.browsers_path.as_deref(),
            self.local_app_data.clone(),
        )?;
        Ok(NodeLauncher {
            install_dir: self.install_dir.clone(),
            lighthouse_browser: lighthouse_browser(settings.lighthouse_browser_path.as_deref()),
            node_path: settings.node_path,
            browsers_path: browsers,
        })
    }
}

impl Launcher for ConfiguredLauncher {
    async fn launch(&self) -> Result<Launched, SidecarError> {
        let launcher = self.node_launcher()?;
        launcher.launch().await
    }
}

impl Sidecar<ConfiguredLauncher> {
    /// Changed settings: the request timeout applies at once, the paths at the next start.
    pub fn apply_settings(&self, settings: SidecarSettings) {
        self.set_rpc_timeout(settings.rpc_timeout());
        self.launcher().set_settings(settings);
    }
}

impl Launcher for NodeLauncher {
    async fn launch(&self) -> Result<Launched, SidecarError> {
        let node = resolve_node(self.node_path.as_deref(), &self.install_dir, Path::is_file)?;
        let version = verify(&node).await?;
        let script = sidecar_script(&self.install_dir);
        if !script.is_file() {
            return Err(SidecarError::ScriptNotFound(script));
        }
        let mut command = Command::new(&node);
        command
            .no_console_window()
            .arg(&script)
            .current_dir(&self.install_dir)
            .env_clear()
            .envs(sidecar_env(
                std::env::vars_os(),
                &self.browsers_path,
                self.lighthouse_browser.as_deref(),
            ))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let (mut child, job) = spawn_in_job(&mut command).map_err(SidecarError::Spawn)?;
        let job = tied(job);
        let missing = || SidecarError::Spawn(io::Error::other("a stdio pipe is missing"));
        let writer = child.stdin.take().ok_or_else(missing)?;
        let reader = child.stdout.take().ok_or_else(missing)?;
        let stderr = child.stderr.take().ok_or_else(missing)?;
        tokio::spawn(log_stderr(stderr));
        tracing::info!(
            target: LOG_TARGET,
            node = %node.display(),
            %version,
            pid = child.id(),
            tree_tied = job.is_some(),
            "the sidecar process started"
        );
        Ok(Launched {
            reader: Box::new(reader),
            writer: Box::new(writer),
            child: Some(child),
            job,
        })
    }
}

/// The sidecar's own job object (rb-process): Node starts suspended, joins it and only then
/// runs, so every process it starts ends with it - however Node or rBrowser end (RB-080-67,
/// review 9). Outside Windows there is none; a failure on Windows is a warning here and, through
/// `rb_process::untied_spawns`, in the app's status bar; the sidecar runs without.
fn tied(job: io::Result<ProcessJob>) -> Option<ProcessJob> {
    match job {
        Ok(job) => Some(job),
        Err(error) if error.kind() == io::ErrorKind::Unsupported => None,
        Err(error) => {
            tracing::warn!(
                target: LOG_TARGET,
                %error,
                "the sidecar's processes are not tied to its life"
            );
            None
        }
    }
}

/// Logs each stderr line of the sidecar at `info`, secrets masked, until the stream ends.
pub async fn log_stderr<R: AsyncRead + Unpin>(stderr: R) {
    let mut reader = BufReader::new(stderr);
    let mut buf = Vec::new();
    loop {
        match read_line(&mut reader, &mut buf, MAX_STDERR_LINE).await {
            Ok(LineRead::Line) => {
                let line = rb_diag::redact(&String::from_utf8_lossy(&buf));
                if !line.trim().is_empty() {
                    tracing::info!(target: LOG_TARGET, "{line}");
                }
            }
            Ok(LineRead::TooLong) => {
                tracing::info!(target: LOG_TARGET, "(a stderr line over 64 KiB was skipped)");
            }
            Ok(LineRead::Eof) | Err(_) => return,
        }
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
