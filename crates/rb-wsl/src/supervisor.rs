//! The helper's lifecycle on the app side: start, handshake, restart after a crash, stop with
//! the app.
//!
//! [`Supervisor::start`] runs a task that starts the helper from a command factory, opens the
//! handshake and publishes a [`HelperStatus`]. A helper that ends on its own is started again
//! after the [`RestartPolicy`]'s wait, until it crashes too often in a row; a protocol mismatch
//! is never retried (the other package will not change). [`Supervisor::restart`] is the
//! `wsl.restart_helper` command; [`Supervisor::shutdown`] sends `shutdown` and kills the helper
//! after a grace time. Dropping the supervisor stops the helper too: the task sees its control
//! channel close, and the child is killed on drop.
//!
//! [`Supervisor::start_session`] gives the helper work (RB-030-05): the [`Session`]'s greeting
//! (`watch` with the rules) is sent after every good handshake, a restart included, and every
//! message the helper writes afterwards (`changes`, `rescan`) goes to the session's channel.

use std::process::Stdio;
use std::time::{Duration, Instant};

use rb_process::NoConsoleWindow;
use serde::Serialize;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::WslError;
use crate::helper::{RestartPolicy, RestartTracker};
use crate::protocol::{HandshakeError, Message, encode, hello, new_nonce};
use crate::supervisor_io::{forward_messages, keep_stderr, read_welcome};

/// How long a failed start waits for the helper's error output to be read.
const STDERR_WAIT: Duration = Duration::from_millis(500);

/// The helper's state, as `wsl_info` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum HelperStatus {
    /// Not running: never started, or stopped.
    Stopped,
    /// Started, the handshake not done yet.
    Starting {
        /// Restarts since [`Supervisor::start`].
        restarts: u32,
    },
    /// Running after a good handshake.
    Running {
        /// The process id of the `wsl.exe` that runs it.
        pid: Option<u32>,
        /// The helper's version from the handshake.
        helper_version: String,
        /// Restarts since [`Supervisor::start`].
        restarts: u32,
    },
    /// Crashed; started again after `delay_ms`.
    Restarting {
        /// Restarts since [`Supervisor::start`], this one included.
        restarts: u32,
        /// The wait before it.
        delay_ms: u64,
    },
    /// Given up until a restart is asked for.
    Failed {
        /// The i18n key of the reason.
        reason_key: String,
        /// The reason for the log.
        detail: String,
        /// Restarts since [`Supervisor::start`].
        restarts: u32,
    },
}

/// The timings of a [`Supervisor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorConfig {
    /// When a crashed helper is started again.
    pub policy: RestartPolicy,
    /// How long the handshake may take; a stopped distribution boots within it.
    pub handshake_timeout: Duration,
    /// How long a helper may take to end after `shutdown` before it is killed.
    pub shutdown_grace: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            policy: RestartPolicy::default(),
            handshake_timeout: Duration::from_secs(30),
            shutdown_grace: Duration::from_secs(2),
        }
    }
}

/// What a supervised helper is given after its handshake.
#[derive(Debug, Default)]
pub struct Session {
    /// Sent after every good handshake, a restart included.
    pub greeting: Vec<Message>,
    /// Gets every message the helper writes after its handshake; `None` drops them.
    pub messages: Option<mpsc::UnboundedSender<Message>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Restart,
    Stop,
}

/// Keeps one helper running.
#[derive(Debug)]
pub struct Supervisor {
    status: watch::Receiver<HelperStatus>,
    control: mpsc::Sender<Control>,
    task: JoinHandle<()>,
}

impl Supervisor {
    /// Starts the helper from `factory` (on the current tokio runtime) and keeps it running.
    pub fn start<F>(factory: F, config: SupervisorConfig) -> Self
    where
        F: FnMut() -> Command + Send + 'static,
    {
        Self::start_session(factory, config, Session::default())
    }

    /// [`Self::start`] with `session`: its greeting after every handshake, the helper's messages
    /// into its channel.
    pub fn start_session<F>(factory: F, config: SupervisorConfig, session: Session) -> Self
    where
        F: FnMut() -> Command + Send + 'static,
    {
        let (status_tx, status) = watch::channel(HelperStatus::Stopped);
        let (control, control_rx) = mpsc::channel(8);
        let task = tokio::spawn(supervise(factory, config, session, status_tx, control_rx));
        Self {
            status,
            control,
            task,
        }
    }

    /// The state now.
    #[must_use]
    pub fn status(&self) -> HelperStatus {
        self.status.borrow().clone()
    }

    /// The state as it changes.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<HelperStatus> {
        self.status.clone()
    }

    /// Ends the helper and starts it again, also after it was given up.
    pub fn restart(&self) {
        // A full queue holds a restart already; a closed one means the task ended.
        let _ = self.control.try_send(Control::Restart);
    }

    /// Ends the helper (`shutdown`, then kill after the grace time) and waits for it.
    pub async fn shutdown(self) {
        let _ = self.control.send(Control::Stop).await;
        if let Err(error) = self.task.await {
            tracing::warn!(%error, "the WSL helper supervisor ended abnormally");
        }
    }
}

/// How one run of the helper ended.
enum Outcome {
    /// Stop asked for, or the supervisor dropped.
    Stopped,
    /// A restart asked for.
    RestartAsked,
    /// Ended on its own; `ran` when the handshake had succeeded.
    Crashed { ran: bool, detail: String },
    /// Not to be retried.
    Fatal(WslError),
}

async fn supervise<F>(
    mut factory: F,
    config: SupervisorConfig,
    session: Session,
    status: watch::Sender<HelperStatus>,
    mut control: mpsc::Receiver<Control>,
) where
    F: FnMut() -> Command + Send + 'static,
{
    let mut tracker = RestartTracker::new(config.policy);
    let mut restarts = 0u32;
    let mut ever_ran = false;
    loop {
        status.send_replace(HelperStatus::Starting { restarts });
        let outcome = run_once(
            factory(),
            &config,
            &session,
            &status,
            restarts,
            &mut control,
        )
        .await;
        let failure = match outcome {
            Outcome::Stopped => break,
            Outcome::RestartAsked => {
                tracker.reset();
                restarts = restarts.saturating_add(1);
                continue;
            }
            Outcome::Fatal(error) => error,
            Outcome::Crashed { ran, detail } => {
                ever_ran |= ran;
                tracing::info!(%detail, "the WSL helper ended");
                if let Some(delay) = tracker.on_crash(Instant::now()) {
                    restarts = restarts.saturating_add(1);
                    status.send_replace(HelperStatus::Restarting {
                        restarts,
                        delay_ms: u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
                    });
                    tokio::select! {
                        () = tokio::time::sleep(delay) => continue,
                        asked = control.recv() => match asked {
                            Some(Control::Restart) => {
                                tracker.reset();
                                continue;
                            }
                            Some(Control::Stop) | None => break,
                        },
                    }
                }
                if ever_ran {
                    WslError::HelperCrashed(tracker.recent())
                } else {
                    WslError::HelperStart(detail)
                }
            }
        };
        tracing::warn!(error = %failure, "the WSL helper is given up");
        status.send_replace(HelperStatus::Failed {
            reason_key: failure.i18n_key().to_owned(),
            detail: failure.to_string(),
            restarts,
        });
        match control.recv().await {
            Some(Control::Restart) => {
                tracker.reset();
                restarts = restarts.saturating_add(1);
            }
            Some(Control::Stop) | None => break,
        }
    }
    status.send_replace(HelperStatus::Stopped);
}

/// Starts `command`, does the handshake and waits for the helper to end or for a control.
async fn run_once(
    mut command: Command,
    config: &SupervisorConfig,
    session: &Session,
    status: &watch::Sender<HelperStatus>,
    restarts: u32,
    control: &mut mpsc::Receiver<Control>,
) -> Outcome {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .no_console_window();
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let detail = if error.kind() == std::io::ErrorKind::NotFound {
                WslError::NotInstalled.to_string()
            } else {
                error.to_string()
            };
            return Outcome::Crashed { ran: false, detail };
        }
    };
    let (stderr, stderr_reader) = keep_stderr(&mut child);
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Outcome::Crashed {
            ran: false,
            detail: "the helper's pipes are missing".into(),
        };
    };
    let mut stdout = BufReader::new(stdout);
    let nonce = new_nonce();
    let handshake = async {
        stdin
            .write_all(encode(&hello(&nonce)).as_bytes())
            .await
            .map_err(|_| HandshakeError::Closed)?;
        stdin.flush().await.map_err(|_| HandshakeError::Closed)?;
        read_welcome(&mut stdout, &nonce).await
    };
    let answer = tokio::select! {
        answer = tokio::time::timeout(config.handshake_timeout, handshake) => {
            answer.unwrap_or(Err(HandshakeError::Timeout))
        }
        asked = control.recv() => {
            return end_on_control(asked, &mut child, None, config).await;
        }
    };
    let helper_version = match answer {
        Ok(version) => version,
        Err(error @ HandshakeError::ProtocolMismatch { .. }) => {
            let _ = child.kill().await;
            return Outcome::Fatal(error.into());
        }
        Err(error) => {
            let _ = child.kill().await;
            // The error output names why (`execvpe(...) failed`); let the reader catch up.
            if let Some(reader) = stderr_reader {
                let _ = tokio::time::timeout(STDERR_WAIT, reader).await;
            }
            let kept = stderr.text();
            let detail = if kept.is_empty() {
                error.to_string()
            } else {
                kept
            };
            return Outcome::Crashed { ran: false, detail };
        }
    };
    status.send_replace(HelperStatus::Running {
        pid: child.id(),
        helper_version,
        restarts,
    });
    for message in &session.greeting {
        // A helper that cannot take its greeting has ended; `wait` below says so.
        if stdin.write_all(encode(message).as_bytes()).await.is_err() {
            break;
        }
    }
    let _ = stdin.flush().await;
    // Without a sink the output stays open (and unread) for the run, as before RB-030-05.
    let (_forwarder, _stdout) = match session.messages.clone() {
        Some(sink) => (Some(AbortOnDrop(forward_messages(stdout, sink))), None),
        None => (None, Some(stdout)),
    };
    tokio::select! {
        ended = child.wait() => {
            let detail = match ended {
                Ok(status) => format!("ended with {status}"),
                Err(error) => error.to_string(),
            };
            Outcome::Crashed { ran: true, detail }
        }
        asked = control.recv() => end_on_control(asked, &mut child, Some(stdin), config).await,
    }
}

/// Ends the reader of a run with the run.
struct AbortOnDrop(JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Ends the helper for a control (or the closed channel): `shutdown` and the grace time for a
/// stop when the handshake is done, a kill otherwise.
async fn end_on_control(
    asked: Option<Control>,
    child: &mut Child,
    stdin: Option<ChildStdin>,
    config: &SupervisorConfig,
) -> Outcome {
    let stop = !matches!(asked, Some(Control::Restart));
    if stop && let Some(mut stdin) = stdin {
        let _ = stdin.write_all(encode(&Message::Shutdown).as_bytes()).await;
        let _ = stdin.flush().await;
        drop(stdin);
        if tokio::time::timeout(config.shutdown_grace, child.wait())
            .await
            .is_ok()
        {
            return Outcome::Stopped;
        }
    }
    let _ = child.kill().await;
    if stop {
        Outcome::Stopped
    } else {
        Outcome::RestartAsked
    }
}

#[cfg(test)]
#[path = "supervisor_tests.rs"]
mod tests;
