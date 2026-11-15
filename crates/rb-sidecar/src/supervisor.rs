//! The sidecar's life: started on the first request (never before), checked with
//! `sidecar.ping`, restarted with backoff after a crash, stopped with rBrowser. A crash ends the
//! running requests with [`SidecarError::Exited`]; the next request starts a new process.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rb_process::ProcessJob;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::Child;
use tokio::sync::{Mutex, broadcast, mpsc, watch};

use crate::LOG_TARGET;
use crate::backoff::{CrashHistory, RestartPolicy};
use crate::client::RpcClient;
use crate::error::SidecarError;
use crate::rpc::Notification;

/// The health check after each start.
pub const PING: &str = "sidecar.ping";
/// Node, Playwright and the installed browsers (diagnostics, About).
pub const VERSIONS: &str = "sidecar.versions";
/// How long a fresh process may take to answer [`PING`].
const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a process may take to end on its own after its stdin closed: the sidecar closes
/// WebKit's windows first (`sidecar/src/main.ts` gives that 3 s). Then it is killed.
pub const STOP_GRACE: Duration = Duration::from_secs(3);
/// How long a stopped process may take to end after it was killed.
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// A started sidecar: its stdout, its stdin and, for a real process, the child and its job.
pub struct Launched {
    /// The sidecar's stdout.
    pub reader: Box<dyn AsyncRead + Send + Unpin>,
    /// The sidecar's stdin.
    pub writer: Box<dyn AsyncWrite + Send + Unpin>,
    /// The process; `None` for a fake in tests.
    pub child: Option<Child>,
    /// The job of the process's whole tree (Windows; `None` elsewhere and for a fake): what
    /// Node starts - Lighthouse's browsers, WebKit, flow workers - ends with the sidecar.
    pub job: Option<ProcessJob>,
}

/// Starts one sidecar process ([`crate::process::NodeLauncher`]; a fake in tests).
pub trait Launcher: Send + Sync {
    /// Starts it.
    fn launch(&self) -> impl Future<Output = Result<Launched, SidecarError>> + Send;
}

struct Running {
    client: RpcClient,
    child: Option<Child>,
    job: Option<ProcessJob>,
}

impl Running {
    /// Ends what is left of the process's tree once Node has ended - an orphan Node could not
    /// end, like a browser child whose parent exited (RB-080-67) - and logs how many there were.
    /// Without a job (outside Windows) the tree is the sidecar's own business.
    fn end_tree(&mut self) {
        let Some(job) = self.job.take() else {
            return;
        };
        let left = job.active_processes();
        if left.is_some_and(|left| left > 0)
            && let Err(error) = job.terminate()
        {
            tracing::warn!(target: LOG_TARGET, %error, "the sidecar's process tree did not end");
        }
        // `left`: the processes still running after Node had ended, ended with the job.
        tracing::info!(
            target: LOG_TARGET,
            left = left.unwrap_or(0),
            "the sidecar's process tree ended"
        );
    }

    /// Asks the process to end - its stdin closes - and kills it when it has not ended after
    /// [`STOP_GRACE`] (`sidecar.restart`, rBrowser quits). The tree ends in every case.
    async fn stop(mut self) -> Option<std::process::ExitStatus> {
        self.client.close_input();
        let ended = match self.child.as_mut() {
            Some(child) => tokio::time::timeout(STOP_GRACE, child.wait())
                .await
                .ok()
                .and_then(Result::ok),
            None => None,
        };
        let status = match ended {
            Some(status) => Some(status),
            None => self.kill_child().await,
        };
        self.end_tree();
        status
    }

    /// Kills the process at once (it crashed or does not answer); the tree ends with it.
    async fn kill(mut self) -> Option<std::process::ExitStatus> {
        self.client.close_input();
        let status = self.kill_child().await;
        self.end_tree();
        status
    }

    /// Kills Node, if there is a process; its status once it ended.
    async fn kill_child(&mut self) -> Option<std::process::ExitStatus> {
        let child = self.child.as_mut()?;
        let _ = child.start_kill();
        tokio::time::timeout(STOP_TIMEOUT, child.wait())
            .await
            .ok()
            .and_then(Result::ok)
    }
}

#[derive(Default)]
struct State {
    running: Option<Running>,
    history: CrashHistory,
    gave_up: Option<u32>,
    /// No start before this: the backoff after a crash or a failed start.
    not_before: Option<Instant>,
    /// rBrowser quits: nothing starts any more.
    shut_down: bool,
}

/// The notifications of one sidecar process ([`Sidecar::subscribe`]).
pub struct Subscription {
    notifications: broadcast::Receiver<Notification>,
    closed: watch::Receiver<bool>,
}

impl Subscription {
    /// The next notification; `None` once the process ended and every notification it sent was
    /// handed out. Notifications missed by a slow reader are skipped with a warning.
    pub async fn recv(&mut self) -> Option<Notification> {
        use broadcast::error::{RecvError, TryRecvError};
        loop {
            tokio::select! {
                biased;
                next = self.notifications.recv() => match next {
                    Ok(notification) => return Some(notification),
                    Err(RecvError::Lagged(missed)) => {
                        tracing::warn!(target: LOG_TARGET, missed, "sidecar notifications dropped");
                    }
                    Err(RecvError::Closed) => return None,
                },
                _ = self.closed.wait_for(|closed| *closed) => {
                    return match self.notifications.try_recv() {
                        Ok(notification) => Some(notification),
                        Err(TryRecvError::Lagged(_)) => continue,
                        Err(_) => None,
                    };
                }
            }
        }
    }
}

impl State {
    /// Whether a start may happen at all.
    fn may_start(&self) -> Result<(), SidecarError> {
        if self.shut_down {
            return Err(SidecarError::ShutDown);
        }
        match self.gave_up {
            Some(crashes) => Err(SidecarError::CrashLoop { crashes }),
            None => Ok(()),
        }
    }

    /// The client of a process that runs and answers.
    fn live(&self) -> Option<RpcClient> {
        let running = self.running.as_ref()?;
        (!running.client.is_closed()).then(|| running.client.clone())
    }
}

/// The one sidecar of the rBrowser process. The state lock is held only for short checks and
/// changes, never across a backoff, a launch or a ping: `shutdown`, `is_running` and requests to
/// a running sidecar never wait for a start. Starts take `starting`, one at a time.
pub struct Sidecar<L> {
    launcher: L,
    /// `engines.nodeRpcTimeoutMs`, changeable while the sidecar runs.
    rpc_timeout_ms: AtomicU64,
    policy: RestartPolicy,
    state: Mutex<State>,
    starting: Mutex<()>,
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Now on tokio's clock (paused in tests), as a `std` instant.
fn now() -> Instant {
    tokio::time::Instant::now().into_std()
}

impl<L: Launcher> Sidecar<L> {
    /// A sidecar that starts nothing yet; `rpc_timeout` is `engines.nodeRpcTimeoutMs`.
    pub fn new(launcher: L, rpc_timeout: Duration) -> Self {
        Self {
            launcher,
            rpc_timeout_ms: AtomicU64::new(millis(rpc_timeout)),
            policy: RestartPolicy::default(),
            state: Mutex::new(State::default()),
            starting: Mutex::new(()),
        }
    }

    /// Other restart rules.
    #[must_use]
    pub fn with_policy(mut self, policy: RestartPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// The launcher.
    pub fn launcher(&self) -> &L {
        &self.launcher
    }

    /// The timeout of a request.
    pub fn rpc_timeout(&self) -> Duration {
        Duration::from_millis(self.rpc_timeout_ms.load(Ordering::Relaxed))
    }

    /// A new timeout for the requests from now on (the setting changed).
    pub fn set_rpc_timeout(&self, timeout: Duration) {
        self.rpc_timeout_ms
            .store(millis(timeout), Ordering::Relaxed);
    }

    /// Whether a process runs and answers.
    pub async fn is_running(&self) -> bool {
        self.state.lock().await.live().is_some()
    }

    /// A request with the configured timeout; starts the sidecar when none runs.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, SidecarError> {
        let client = self.client().await?;
        client
            .request(method, params, Some(self.rpc_timeout()))
            .await
    }

    /// A request with its own timeout (`None`: none) and its `$/progress` sent to `progress`.
    pub async fn request_with_progress(
        &self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
        progress: mpsc::UnboundedSender<Value>,
    ) -> Result<Value, SidecarError> {
        let client = self.client().await?;
        client
            .request_with_progress(method, params, timeout, progress)
            .await
    }

    /// [`VERSIONS`] when a sidecar runs; `None` otherwise - the diagnostics never start Node.
    pub async fn versions_if_running(&self) -> Option<Value> {
        let client = self.state.lock().await.live()?;
        client
            .request(VERSIONS, Value::Null, Some(self.rpc_timeout()))
            .await
            .ok()
    }

    /// A request with the configured timeout, only when a sidecar runs; `None` otherwise - for
    /// closing what a sidecar holds (WebKit's windows, RB-040-04) without starting a new one.
    pub async fn request_if_running(
        &self,
        method: &str,
        params: Value,
    ) -> Option<Result<Value, SidecarError>> {
        let client = self.state.lock().await.live()?;
        Some(
            client
                .request(method, params, Some(self.rpc_timeout()))
                .await,
        )
    }

    /// The notifications of the running sidecar (starting it when none runs), from now on. The
    /// subscription ends when that process ends; a restarted sidecar needs a new one.
    pub async fn subscribe(&self) -> Result<Subscription, SidecarError> {
        let client = self.client().await?;
        Ok(Subscription {
            notifications: client.subscribe(),
            closed: client.closed_watch(),
        })
    }

    /// Stops the process if one runs and starts a new one, forgetting earlier crashes
    /// (`sidecar.restart`). Refused once rBrowser shuts down.
    pub async fn restart(&self) -> Result<(), SidecarError> {
        let _starting = self.starting.lock().await;
        let old = {
            let mut state = self.state.lock().await;
            if state.shut_down {
                return Err(SidecarError::ShutDown);
            }
            state.history.reset();
            state.gave_up = None;
            state.not_before = None;
            state.running.take()
        };
        if let Some(old) = old {
            old.stop().await;
        }
        self.start().await.map(|_| ())
    }

    /// Stops the process (rBrowser quits); nothing starts afterwards. A start under way stops
    /// what it started.
    pub async fn shutdown(&self) {
        let running = {
            let mut state = self.state.lock().await;
            state.shut_down = true;
            state.running.take()
        };
        if let Some(running) = running {
            running.stop().await;
            tracing::info!(target: LOG_TARGET, "the sidecar was stopped");
        }
    }

    async fn client(&self) -> Result<RpcClient, SidecarError> {
        {
            let state = self.state.lock().await;
            state.may_start()?;
            if let Some(client) = state.live() {
                return Ok(client);
            }
        }
        let _starting = self.starting.lock().await;
        // Another request may have started one, or rBrowser quit, while this one waited.
        let crashed = {
            let mut state = self.state.lock().await;
            state.may_start()?;
            if let Some(client) = state.live() {
                return Ok(client);
            }
            state.running.take()
        };
        if let Some(crashed) = crashed {
            let died = crashed.client.closed_at().unwrap_or_else(now);
            let status = crashed.kill().await;
            let status = status.map_or_else(|| "unknown".to_owned(), |s| s.to_string());
            tracing::warn!(target: LOG_TARGET, %status, "the sidecar exited unexpectedly");
            let mut state = self.state.lock().await;
            match state.history.crashed(&self.policy, died) {
                Ok(delay) => state.not_before = Some(died + delay),
                Err(crashes) => {
                    tracing::error!(
                        target: LOG_TARGET,
                        crashes,
                        "the sidecar keeps crashing; it starts again only by hand"
                    );
                    state.gave_up = Some(crashes);
                    return Err(SidecarError::CrashLoop { crashes });
                }
            }
        }
        let not_before = self.state.lock().await.not_before;
        if let Some(at) = not_before {
            tokio::time::sleep_until(tokio::time::Instant::from_std(at)).await;
        }
        self.state.lock().await.may_start()?;
        self.start().await
    }

    /// Launches and pings a new process, without the state lock; called under `starting`. A
    /// failed start counts like a crash.
    async fn start(&self) -> Result<RpcClient, SidecarError> {
        let launched = match self.launcher.launch().await {
            Ok(launched) => launched,
            Err(error) => return Err(self.failed_start(error).await),
        };
        let begun = now();
        let client = RpcClient::start(launched.reader, launched.writer);
        let running = Running {
            client: client.clone(),
            child: launched.child,
            job: launched.job,
        };
        if let Err(error) = client
            .request(PING, Value::Null, Some(HEALTH_TIMEOUT))
            .await
        {
            running.kill().await;
            let error = SidecarError::Unhealthy(rb_diag::redact(&error.to_string()));
            return Err(self.failed_start(error).await);
        }
        let mut state = self.state.lock().await;
        if state.shut_down {
            drop(state);
            running.stop().await;
            return Err(SidecarError::ShutDown);
        }
        let ms = u64::try_from(begun.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(target: LOG_TARGET, ms, "the sidecar started and answered its ping");
        state.history.started(begun);
        state.not_before = None;
        state.running = Some(running);
        Ok(client)
    }

    /// Records a failed start: the backoff before the next one, or the end of automatic starts.
    /// Returns `error`.
    async fn failed_start(&self, error: SidecarError) -> SidecarError {
        let mut state = self.state.lock().await;
        if state.shut_down {
            return error;
        }
        let failed = now();
        match state.history.failed_start(&self.policy) {
            Ok(delay) => state.not_before = Some(failed + delay),
            Err(crashes) => {
                tracing::error!(
                    target: LOG_TARGET,
                    crashes,
                    "the sidecar keeps failing to start; it starts again only by hand"
                );
                state.gave_up = Some(crashes);
            }
        }
        error
    }
}

#[cfg(test)]
#[path = "supervisor_tests.rs"]
mod tests;
