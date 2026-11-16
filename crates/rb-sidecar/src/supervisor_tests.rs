//! The supervisor against fake sidecars on in-memory pipes.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

use super::*;

/// How one fake sidecar behaves.
#[derive(Clone, Copy, Debug)]
enum Mode {
    /// Answers everything.
    Normal,
    /// Never answers, not even the health ping.
    Mute,
    /// Answers the health ping, then dies on the next request.
    CrashAfterPing,
    /// Answers the health ping after 5 s.
    SlowPing,
    /// Answers the health ping, then dies 1 s later on its own.
    ExitSoon,
    /// The launch itself fails.
    FailLaunch,
}

#[derive(Default)]
struct FakeLauncher {
    launches: AtomicUsize,
    /// The modes of the next launches; `Normal` when empty.
    modes: Mutex<VecDeque<Mode>>,
}

impl FakeLauncher {
    fn with(modes: &[Mode]) -> Self {
        Self {
            launches: AtomicUsize::new(0),
            modes: Mutex::new(modes.iter().copied().collect()),
        }
    }

    fn launches(&self) -> usize {
        self.launches.load(Ordering::SeqCst)
    }
}

impl Launcher for &FakeLauncher {
    async fn launch(&self) -> Result<Launched, SidecarError> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        let mode = self
            .modes
            .lock()
            .expect("not poisoned")
            .pop_front()
            .unwrap_or(Mode::Normal);
        if matches!(mode, Mode::FailLaunch) {
            return Err(SidecarError::Spawn(std::io::Error::other("no node")));
        }
        let (client_out, fake_in) = tokio::io::duplex(4096);
        let (fake_out, client_in) = tokio::io::duplex(4096);
        tokio::spawn(fake_sidecar(mode, fake_in, fake_out));
        Ok(Launched {
            reader: Box::new(client_in),
            writer: Box::new(client_out),
            child: None,
            job: None,
        })
    }
}

async fn fake_sidecar(mode: Mode, input: DuplexStream, mut output: DuplexStream) {
    let mut lines = BufReader::new(input).lines();
    let mut pinged = false;
    while let Ok(Some(line)) = lines.next_line().await {
        let request: Value = serde_json::from_str(&line).expect("JSON");
        let Some(id) = request.get("id") else {
            continue;
        };
        let method = request["method"].as_str().unwrap_or_default();
        let result = match (mode, method) {
            (Mode::Mute, _) => continue,
            (Mode::CrashAfterPing, _) if pinged => return,
            (_, "crash") => return,
            (_, "hang") => continue,
            (Mode::ExitSoon, PING) => {
                let answer = json!({ "jsonrpc": "2.0", "id": id, "result": { "pong": true } });
                let line = format!("{answer}\n");
                let _ = output.write_all(line.as_bytes()).await;
                tokio::time::sleep(Duration::from_secs(1)).await;
                return;
            }
            (Mode::SlowPing, PING) => {
                tokio::time::sleep(Duration::from_secs(5)).await;
                json!({ "pong": true })
            }
            (_, PING) => {
                pinged = true;
                json!({ "pong": true })
            }
            (_, VERSIONS) => json!({ "node": "26.13.0" }),
            (_, "notify") => {
                let event =
                    json!({ "jsonrpc": "2.0", "method": "test.event", "params": { "n": 1 } });
                let line = format!("{event}\n");
                if output.write_all(line.as_bytes()).await.is_err() {
                    return;
                }
                json!("notified")
            }
            (_, other) => json!(other),
        };
        let mut answer = json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string();
        answer.push('\n');
        if output.write_all(answer.as_bytes()).await.is_err() {
            return;
        }
    }
}

fn policy() -> RestartPolicy {
    RestartPolicy {
        base: Duration::from_millis(100),
        max_delay: Duration::from_secs(1),
        max_crashes: 2,
        stable_after: Duration::from_secs(60),
    }
}

fn sidecar(launcher: &FakeLauncher) -> Sidecar<&FakeLauncher> {
    Sidecar::new(launcher, Duration::from_secs(5)).with_policy(policy())
}

#[tokio::test]
async fn nothing_starts_before_the_first_request() {
    let launcher = FakeLauncher::default();
    let sidecar = sidecar(&launcher);
    assert!(!sidecar.is_running().await);
    assert_eq!(sidecar.versions_if_running().await, None);
    assert_eq!(launcher.launches(), 0);
    assert_eq!(
        sidecar.request("echo", Value::Null).await.expect("answer"),
        json!("echo")
    );
    assert_eq!(launcher.launches(), 1);
    assert!(sidecar.is_running().await);
    assert_eq!(
        sidecar.versions_if_running().await,
        Some(json!({ "node": "26.13.0" }))
    );
    sidecar.request("echo", Value::Null).await.expect("answer");
    assert_eq!(launcher.launches(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_crash_ends_the_running_request_and_the_next_one_restarts() {
    let launcher = FakeLauncher::default();
    let sidecar = sidecar(&launcher);
    let error = sidecar
        .request("crash", Value::Null)
        .await
        .expect_err("crashed");
    assert!(matches!(error, SidecarError::Exited));
    assert!(!sidecar.is_running().await);
    assert_eq!(
        sidecar
            .request("echo", Value::Null)
            .await
            .expect("restarted"),
        json!("echo")
    );
    assert_eq!(launcher.launches(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_crash_loop_stops_until_a_restart_by_hand() {
    let launcher = FakeLauncher::with(&[Mode::CrashAfterPing; 3]);
    let sidecar = sidecar(&launcher);
    for _ in 0..3 {
        let error = sidecar.request("a", Value::Null).await.expect_err("crash");
        assert!(matches!(error, SidecarError::Exited), "{error}");
    }
    for _ in 0..2 {
        let error = sidecar
            .request("a", Value::Null)
            .await
            .expect_err("gave up");
        assert!(
            matches!(error, SidecarError::CrashLoop { crashes: 3 }),
            "{error}"
        );
    }
    assert_eq!(launcher.launches(), 3);
    sidecar.restart().await.expect("restarts");
    assert_eq!(launcher.launches(), 4);
    assert_eq!(
        sidecar.request("a", Value::Null).await.expect("answer"),
        json!("a")
    );
}

#[tokio::test(start_paused = true)]
async fn a_sidecar_without_a_ping_answer_is_unhealthy() {
    let launcher = FakeLauncher::with(&[Mode::Mute]);
    let sidecar = sidecar(&launcher);
    let error = sidecar
        .request("a", Value::Null)
        .await
        .expect_err("unhealthy");
    assert!(matches!(error, SidecarError::Unhealthy(_)), "{error}");
    assert!(!sidecar.is_running().await);
    assert_eq!(
        sidecar
            .request("a", Value::Null)
            .await
            .expect("second start"),
        json!("a")
    );
}

#[tokio::test]
async fn shutdown_stops_and_nothing_starts_afterwards() {
    let launcher = FakeLauncher::default();
    let sidecar = sidecar(&launcher);
    sidecar.request("a", Value::Null).await.expect("answer");
    sidecar.shutdown().await;
    assert!(!sidecar.is_running().await);
    let error = sidecar
        .request("a", Value::Null)
        .await
        .expect_err("shut down");
    assert!(matches!(error, SidecarError::ShutDown), "{error}");
    let error = sidecar.restart().await.expect_err("shut down");
    assert!(matches!(error, SidecarError::ShutDown), "{error}");
    assert_eq!(launcher.launches(), 1);
}

#[tokio::test(start_paused = true)]
async fn shutdown_does_not_wait_for_the_backoff_of_a_restart() {
    let launcher = FakeLauncher::default();
    let sidecar = Sidecar::new(&launcher, Duration::from_secs(5)).with_policy(RestartPolicy {
        base: Duration::from_secs(10),
        max_delay: Duration::from_secs(30),
        ..policy()
    });
    let error = sidecar
        .request("crash", Value::Null)
        .await
        .expect_err("crashed");
    assert!(matches!(error, SidecarError::Exited), "{error}");
    let (request, stopped) = tokio::join!(sidecar.request("a", Value::Null), async {
        tokio::task::yield_now().await;
        let stopped = tokio::time::timeout(Duration::from_secs(1), sidecar.shutdown()).await;
        let running = tokio::time::timeout(Duration::from_secs(1), sidecar.is_running()).await;
        (stopped, running)
    });
    assert!(stopped.0.is_ok(), "shutdown waited for the backoff");
    assert_eq!(
        stopped.1.ok(),
        Some(false),
        "is_running waited for the backoff"
    );
    let error = request.expect_err("shut down during the backoff");
    assert!(matches!(error, SidecarError::ShutDown), "{error}");
    assert_eq!(launcher.launches(), 1);
}

#[tokio::test(start_paused = true)]
async fn shutdown_does_not_wait_for_a_start_and_stops_what_it_started() {
    let launcher = FakeLauncher::with(&[Mode::SlowPing]);
    let sidecar = sidecar(&launcher);
    let (request, stopped) = tokio::join!(sidecar.request("a", Value::Null), async {
        tokio::task::yield_now().await;
        tokio::time::timeout(Duration::from_secs(1), sidecar.shutdown()).await
    });
    assert!(stopped.is_ok(), "shutdown waited for the start");
    let error = request.expect_err("shut down during the start");
    assert!(matches!(error, SidecarError::ShutDown), "{error}");
    assert!(!sidecar.is_running().await);
    assert_eq!(launcher.launches(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_crash_counts_from_its_death_not_from_the_next_request() {
    let launcher = FakeLauncher::with(&[Mode::ExitSoon; 3]);
    let sidecar = sidecar(&launcher);
    for _ in 0..2 {
        let error = sidecar.request("a", Value::Null).await.expect_err("died");
        assert!(matches!(error, SidecarError::Exited), "{error}");
        // Each run lasted 1 s; the next request comes long after the stable time.
        tokio::time::sleep(Duration::from_secs(61)).await;
    }
    let error = sidecar.request("a", Value::Null).await.expect_err("died");
    assert!(matches!(error, SidecarError::Exited), "{error}");
    tokio::time::sleep(Duration::from_secs(61)).await;
    let error = sidecar
        .request("a", Value::Null)
        .await
        .expect_err("gave up");
    assert!(
        matches!(error, SidecarError::CrashLoop { crashes: 3 }),
        "{error}"
    );
    assert_eq!(launcher.launches(), 3);
}

#[tokio::test(start_paused = true)]
async fn failed_starts_back_off_and_count_toward_the_limit() {
    let launcher = FakeLauncher::with(&[Mode::FailLaunch, Mode::Mute, Mode::FailLaunch]);
    let sidecar = sidecar(&launcher);
    let begun = tokio::time::Instant::now();
    let error = sidecar.request("a", Value::Null).await.expect_err("fails");
    assert!(matches!(error, SidecarError::Spawn(_)), "{error}");
    let error = sidecar.request("a", Value::Null).await.expect_err("fails");
    assert!(matches!(error, SidecarError::Unhealthy(_)), "{error}");
    let before_third = tokio::time::Instant::now();
    let error = sidecar.request("a", Value::Null).await.expect_err("fails");
    assert!(matches!(error, SidecarError::Spawn(_)), "{error}");
    // 100 ms before the second start; 200 ms after the second failure.
    assert!(before_third.duration_since(begun) >= Duration::from_millis(100));
    assert!(before_third.elapsed() >= Duration::from_millis(200));
    let error = sidecar
        .request("a", Value::Null)
        .await
        .expect_err("gave up");
    assert!(
        matches!(error, SidecarError::CrashLoop { crashes: 3 }),
        "{error}"
    );
    assert_eq!(launcher.launches(), 3);
}

#[tokio::test(start_paused = true)]
async fn a_changed_timeout_applies_to_the_next_request() {
    let launcher = FakeLauncher::default();
    let sidecar = sidecar(&launcher);
    sidecar.set_rpc_timeout(Duration::from_secs(2));
    let error = sidecar
        .request("hang", Value::Null)
        .await
        .expect_err("times out");
    assert!(
        matches!(error, SidecarError::Timeout { ms: 2_000, .. }),
        "{error}"
    );
}

#[tokio::test]
async fn request_if_running_never_starts_a_sidecar() {
    let launcher = FakeLauncher::default();
    let sidecar = sidecar(&launcher);
    assert!(sidecar.request_if_running("a", Value::Null).await.is_none());
    assert_eq!(launcher.launches(), 0);
    sidecar.request("a", Value::Null).await.expect("answer");
    let answer = sidecar.request_if_running("b", Value::Null).await;
    assert_eq!(answer.map(Result::ok), Some(Some(json!("b"))));
    sidecar.shutdown().await;
    assert!(sidecar.request_if_running("c", Value::Null).await.is_none());
    assert_eq!(launcher.launches(), 1);
}

#[tokio::test]
async fn subscribe_starts_the_sidecar_and_hands_out_its_notifications() {
    let launcher = FakeLauncher::default();
    let sidecar = sidecar(&launcher);
    let mut notifications = sidecar.subscribe().await.expect("started");
    assert_eq!(launcher.launches(), 1);
    sidecar
        .request("notify", Value::Null)
        .await
        .expect("answer");
    let notification = notifications.recv().await.expect("a notification");
    assert_eq!(notification.method, "test.event");
    assert_eq!(notification.params, json!({ "n": 1 }));
    let crashed = sidecar.request("crash", Value::Null).await;
    assert!(matches!(crashed, Err(SidecarError::Exited)));
    assert!(
        notifications.recv().await.is_none(),
        "the subscription ends with the process"
    );
}

/// Quitting closes the sidecar's stdin even while a request holds the connection: the sidecar
/// ends on its own and the request ends with it, instead of waiting for its timeout.
#[tokio::test(start_paused = true)]
async fn shutdown_closes_stdin_while_a_request_runs() {
    let launcher = FakeLauncher::default();
    let sidecar = sidecar(&launcher);
    sidecar.request("a", Value::Null).await.expect("answer");
    let (hanging, ()) = tokio::join!(sidecar.request("hang", Value::Null), async {
        tokio::task::yield_now().await;
        sidecar.shutdown().await;
    });
    let error = hanging.expect_err("ended");
    assert!(matches!(error, SidecarError::Exited), "{error}");
}

/// A stopped sidecar ends on its own once its stdin closes - it is not killed when it does so
/// within the grace (its WebKit windows close first).
#[cfg(unix)]
#[tokio::test]
async fn stop_lets_the_process_end_on_its_own() {
    use rb_process::NoConsoleWindow;
    let mut child = tokio::process::Command::new("sh")
        .args(["-c", "cat > /dev/null; exit 7"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .no_console_window()
        .spawn()
        .expect("sh runs");
    let reader = child.stdout.take().expect("stdout");
    let writer = child.stdin.take().expect("stdin");
    let running = Running {
        client: RpcClient::start(reader, writer),
        child: Some(child),
        job: None,
    };
    let status = tokio::time::timeout(Duration::from_secs(2), running.stop())
        .await
        .expect("ends within the grace")
        .expect("a status");
    assert_eq!(status.code(), Some(7), "ended on its own, not killed");
}
