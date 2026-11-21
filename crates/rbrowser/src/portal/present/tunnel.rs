//! The Cloudflare Quick Tunnel of a presentation (RB-096-05): `cloudflared tunnel
//! --no-autoupdate --config <blank> --url http://127.0.0.1:<port>` - no account, a random
//! `https://<name>.trycloudflare.com` - to the presentation's listener on loopback. `--config`
//! names rBrowser's own blank file ([`BLANK_CONFIG`], beside the program, else in the tools
//! folder, written anew on every start), so the person's `~/.cloudflared/config.yml` (a named
//! tunnel, ingress rules) is never read.
//!
//! The program runs below the app's priority without a console window
//! ([`rb_process::low_priority_command`]), in a job of its own ([`rb_process::spawn_in_job`],
//! on Windows its whole tree ends with rBrowser however rBrowser ends), and is killed when its
//! [`Tunnel`] is stopped or dropped. Its environment loses every `TUNNEL_*` variable (cloudflared
//! reads them as flags: a `TUNNEL_TOKEN` of the person's own would start their named tunnel) and
//! `NO_AUTOUPDATE`. Its output is read line by line for the address ([`public_url`]: only a
//! quick tunnel's name, never `api.trycloudflare.com`) and the first registered connection
//! ([`Progress`]); a line is logged at `debug` only, through the log mask.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt as _, AsyncRead, BufReader};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::cdp::lock;

/// The domain of every quick tunnel.
pub const QUICK_TUNNEL_DOMAIN: &str = ".trycloudflare.com";
/// What cloudflared writes once the edge took the tunnel's first connection.
pub const CONNECTED_MARK: &str = "Registered tunnel connection";
/// How long a stop waits for the program to end after it was killed.
const STOP_WAIT: Duration = Duration::from_secs(10);
/// The longest quick tunnel name taken (a DNS label).
const MAX_LABEL: usize = 63;
/// The name of the blank configuration file cloudflared is pointed at.
pub const BLANK_CONFIG: &str = "rbrowser-cloudflared.yml";
/// Its text: an empty mapping (an empty file makes cloudflared log an error).
const BLANK_CONFIG_TEXT: &str =
    "# Written by rBrowser (RB-096-05): cloudflared reads no configuration of its own.\n{}\n";

/// What the tunnel reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TunnelEvent {
    /// Its public address, `https://<name>.trycloudflare.com`.
    Url(String),
    /// The edge took its first connection: viewers can reach it.
    Connected,
    /// The program ended; `stopped` when rBrowser ended it (a stop or the handle dropped), else
    /// it crashed or quit by itself.
    Exited {
        /// rBrowser ended it.
        stopped: bool,
    },
}

/// The command line after the program, `config` the blank configuration file.
#[must_use]
pub fn arguments(port: u16, config: &Path) -> Vec<OsString> {
    vec![
        OsString::from("tunnel"),
        OsString::from("--no-autoupdate"),
        OsString::from("--config"),
        config.as_os_str().to_owned(),
        OsString::from("--url"),
        OsString::from(format!("http://127.0.0.1:{port}")),
    ]
}

/// Writes the blank configuration into the first of `folders` that takes it (made when
/// missing); its path.
///
/// # Errors
///
/// No folder takes it: the last folder's error.
pub fn blank_config(folders: impl IntoIterator<Item = PathBuf>) -> std::io::Result<PathBuf> {
    let mut failed = std::io::Error::new(std::io::ErrorKind::NotFound, "no folder for the file");
    for folder in folders {
        let path = folder.join(BLANK_CONFIG);
        let written = std::fs::create_dir_all(&folder)
            .and_then(|()| rb_util::fs::write_atomic(&path, BLANK_CONFIG_TEXT.as_bytes()));
        match written {
            Ok(()) => return Ok(path),
            Err(error) => failed = error,
        }
    }
    Err(failed)
}

/// Of the environment variables `keys`, those the program must not see: `TUNNEL_*` and
/// `NO_AUTOUPDATE`, in any case (Windows reads them so).
pub fn scrubbed(keys: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    keys.into_iter()
        .filter(|key| {
            let upper = key.to_string_lossy().to_ascii_uppercase();
            upper.starts_with("TUNNEL_") || upper == "NO_AUTOUPDATE"
        })
        .collect()
}

/// The quick tunnel's address in `line`: `https://` and one DNS label with a dash (quick tunnel
/// names are words joined by dashes) in front of [`QUICK_TUNNEL_DOMAIN`], lower case.
#[must_use]
pub fn public_url(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    lower.match_indices("https://").find_map(|(at, scheme)| {
        let rest = &lower[at + scheme.len()..];
        let host: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
            .collect();
        let label = host.strip_suffix(QUICK_TUNNEL_DOMAIN)?;
        let named = !label.is_empty()
            && label.len() <= MAX_LABEL
            && label.contains('-')
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        named.then(|| format!("https://{host}"))
    })
}

/// What the output said so far.
#[derive(Debug, Default)]
pub struct Progress {
    url: bool,
    connected: bool,
}

impl Progress {
    /// The event `line` makes: the address once, then the first connection once.
    pub fn feed(&mut self, line: &str) -> Option<TunnelEvent> {
        if !self.url {
            let url = public_url(line)?;
            self.url = true;
            return Some(TunnelEvent::Url(url));
        }
        if !self.connected && line.contains(CONNECTED_MARK) {
            self.connected = true;
            return Some(TunnelEvent::Connected);
        }
        None
    }
}

/// A running cloudflared; stopping or dropping it ends the program.
#[derive(Debug)]
pub struct Tunnel {
    pid: Option<u32>,
    stop: Option<oneshot::Sender<()>>,
    done: Option<JoinHandle<()>>,
}

impl Tunnel {
    /// Starts `program` for the listener on `127.0.0.1:port`; its events.
    ///
    /// # Errors
    ///
    /// The blank configuration cannot be written, or the program cannot be started.
    pub fn spawn(
        program: &Path,
        port: u16,
    ) -> std::io::Result<(Self, mpsc::UnboundedReceiver<TunnelEvent>)> {
        // Beside the program (a managed one is in the tools folder), else in the tools folder.
        let beside = program
            .parent()
            .filter(|folder| !folder.as_os_str().is_empty())
            .map(Path::to_path_buf);
        let tools = crate::tools::ToolStore::default_root().map(|root| root.join("cloudflared"));
        let folders = beside.into_iter().chain(tools);
        let config = blank_config(folders)?;
        let mut command = rb_process::low_priority_command(program);
        command
            .args(arguments(port, &config))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for key in scrubbed(std::env::vars_os().map(|(key, _)| key)) {
            command.env_remove(key);
        }
        let (mut child, job) = rb_process::spawn_in_job(&mut command)?;
        if let Err(error) = &job
            && error.kind() != std::io::ErrorKind::Unsupported
        {
            tracing::warn!(%error, "cloudflared runs outside a job of its own");
        }
        let pid = child.id();
        let (events, receiver) = mpsc::unbounded_channel();
        let progress = Arc::new(Mutex::new(Progress::default()));
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(read_lines(stdout, progress.clone(), events.clone()));
        }
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(read_lines(stderr, progress, events.clone()));
        }
        let (stop, stopped) = oneshot::channel::<()>();
        let done = tokio::spawn(async move {
            let by_us = tokio::select! {
                status = child.wait() => {
                    tracing::info!(status = ?status.ok(), "cloudflared ended");
                    false
                }
                _ = stopped => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    true
                }
            };
            // The job ends whatever the program left (on Windows).
            drop(job);
            let _ = events.send(TunnelEvent::Exited { stopped: by_us });
        });
        Ok((
            Self {
                pid,
                stop: Some(stop),
                done: Some(done),
            },
            receiver,
        ))
    }

    /// The program's process id.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Kills the program and waits until it ended.
    pub async fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(done) = self.done.take()
            && tokio::time::timeout(STOP_WAIT, done).await.is_err()
        {
            tracing::warn!("cloudflared did not end in time after it was killed");
        }
    }
}

async fn read_lines(
    stream: impl AsyncRead + Unpin,
    progress: Arc<Mutex<Progress>>,
    events: mpsc::UnboundedSender<TunnelEvent>,
) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        tracing::debug!(line = %rb_diag::redact(&line), "cloudflared");
        let event = lock(&progress).feed(&line);
        if let Some(event) = event {
            let _ = events.send(event);
        }
    }
}

#[cfg(test)]
#[path = "tunnel_tests.rs"]
mod tests;
