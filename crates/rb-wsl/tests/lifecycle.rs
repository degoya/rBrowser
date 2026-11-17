//! The real helper binary under the supervisor: started, killed and started again, stopped; and
//! its own side of a wrong protocol. Unix only: the test kills with `kill`.
#![cfg(unix)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;

use rb_wsl::helper::RestartPolicy;
use rb_wsl::protocol::{ErrorCode, Message, PROTOCOL_VERSION, decode, encode, new_nonce};
use rb_wsl::supervisor::{HelperStatus, Supervisor, SupervisorConfig};
use tokio::process::Command;
use tokio::sync::watch;

const HELPER: &str = env!("CARGO_BIN_EXE_rbrowser-wsl");

fn config() -> SupervisorConfig {
    SupervisorConfig {
        policy: RestartPolicy {
            max_crashes: 3,
            window: Duration::from_secs(60),
            first_delay: Duration::from_millis(20),
            max_delay: Duration::from_millis(50),
        },
        handshake_timeout: Duration::from_secs(10),
        shutdown_grace: Duration::from_secs(2),
    }
}

async fn wait_for(
    status: &mut watch::Receiver<HelperStatus>,
    what: impl FnMut(&HelperStatus) -> bool,
) -> HelperStatus {
    tokio::time::timeout(Duration::from_secs(10), status.wait_for(what))
        .await
        .expect("the state came in time")
        .expect("the supervisor is alive")
        .clone()
}

fn running(status: &HelperStatus) -> Option<(u32, u32)> {
    match status {
        HelperStatus::Running {
            pid: Some(pid),
            restarts,
            ..
        } => Some((*pid, *restarts)),
        _ => None,
    }
}

fn alive(pid: u32) -> bool {
    StdCommand::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[tokio::test]
async fn a_killed_helper_is_started_again() {
    let supervisor = Supervisor::start(
        || {
            let mut command = Command::new(HELPER);
            command.arg("cdp-proxy");
            command
        },
        config(),
    );
    let mut status = supervisor.subscribe();
    let first = wait_for(&mut status, |s| running(s).is_some()).await;
    let HelperStatus::Running { helper_version, .. } = &first else {
        unreachable!("waited for running");
    };
    assert_eq!(helper_version, env!("CARGO_PKG_VERSION"));
    let (pid, restarts) = running(&first).expect("running");
    assert_eq!(restarts, 0);

    let killed = StdCommand::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .expect("kill runs");
    assert!(killed.success());
    let again = wait_for(&mut status, |s| running(s).is_some_and(|(_, r)| r == 1)).await;
    let (new_pid, _) = running(&again).expect("running");
    assert_ne!(new_pid, pid);

    supervisor.restart();
    let restarted = wait_for(&mut status, |s| running(s).is_some_and(|(_, r)| r == 2)).await;
    let (last_pid, _) = running(&restarted).expect("running");

    supervisor.shutdown().await;
    assert_eq!(*status.borrow(), HelperStatus::Stopped);
    assert!(!alive(last_pid), "the helper ended with the supervisor");
}

#[test]
fn the_helper_refuses_another_protocol_clearly() {
    let mut child = StdCommand::new(HELPER)
        .arg("watch")
        .arg("/home/u/app")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the helper starts");
    let hello = Message::Hello {
        protocol: PROTOCOL_VERSION + 1,
        nonce: new_nonce(),
    };
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(encode(&hello).as_bytes())
        .expect("written");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("an answer");
    assert!(matches!(
        decode(&line),
        Ok(Message::Error {
            code: ErrorCode::ProtocolMismatch,
            protocol: Some(PROTOCOL_VERSION),
            ..
        })
    ));
    let output = child.wait_with_output().expect("ended");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("protocol mismatch"), "{stderr}");
}

#[test]
fn version_and_a_bad_watch_path() {
    let output = StdCommand::new(HELPER)
        .arg("--version")
        .output()
        .expect("runs");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("rbrowser-wsl {}", env!("CARGO_PKG_VERSION"))
    );
    let output = StdCommand::new(HELPER)
        .args(["watch", "relative"])
        .stdin(Stdio::null())
        .output()
        .expect("runs");
    assert_eq!(output.status.code(), Some(2));
}
