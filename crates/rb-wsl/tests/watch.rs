//! The real helper's `watch` under the supervisor (RB-030-05): the rules go out after the
//! handshake, the changes of a temp dir come back, and a killed helper gets its rules again.
//! Unix only: inotify, and the test kills with `kill`.
#![cfg(unix)]

use std::process::Command as StdCommand;
use std::time::Duration;

use rb_wsl::helper::RestartPolicy;
use rb_wsl::protocol::Message;
use rb_wsl::supervisor::{HelperStatus, Session, Supervisor, SupervisorConfig};
use rb_wsl::watch::{ChangeKind, WatchRules};
use tokio::process::Command;
use tokio::sync::mpsc;

const HELPER: &str = env!("CARGO_BIN_EXE_rbrowser-wsl");

async fn next(messages: &mut mpsc::UnboundedReceiver<Message>) -> Message {
    tokio::time::timeout(Duration::from_secs(10), messages.recv())
        .await
        .expect("a message in time")
        .expect("the supervisor is alive")
}

#[tokio::test]
async fn the_supervised_helper_watches_a_folder_and_gets_its_rules_after_a_restart() {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::create_dir_all(dir.path().join("node_modules/x")).expect("a folder");
    let root = dir.path().to_str().expect("a UTF-8 temp path").to_owned();
    let (sink, mut messages) = mpsc::unbounded_channel();
    let session = Session {
        greeting: vec![Message::Watch {
            rules: WatchRules {
                debounce_ms: 50,
                ..WatchRules::default()
            },
        }],
        messages: Some(sink),
    };
    let config = SupervisorConfig {
        policy: RestartPolicy {
            max_crashes: 3,
            window: Duration::from_secs(60),
            first_delay: Duration::from_millis(20),
            max_delay: Duration::from_millis(50),
        },
        handshake_timeout: Duration::from_secs(10),
        shutdown_grace: Duration::from_secs(2),
    };
    let supervisor = Supervisor::start_session(
        move || {
            let mut command = Command::new(HELPER);
            command.args(["watch", &root]);
            command
        },
        config,
        session,
    );
    assert_eq!(next(&mut messages).await, Message::Watching { folders: 1 });

    std::fs::write(dir.path().join("node_modules/x/a.js"), "x").expect("written");
    std::fs::write(dir.path().join("app.css"), "a{}").expect("written");
    let Message::Changes { changes } = next(&mut messages).await else {
        panic!("changes");
    };
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].path, "app.css");
    assert!(matches!(
        changes[0].kind,
        ChangeKind::Create | ChangeKind::Modify
    ));

    let HelperStatus::Running { pid: Some(pid), .. } = supervisor.status() else {
        panic!("running");
    };
    StdCommand::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .expect("kill runs");
    assert_eq!(
        next(&mut messages).await,
        Message::Watching { folders: 1 },
        "the restarted helper got its rules again"
    );
    supervisor.shutdown().await;
}
