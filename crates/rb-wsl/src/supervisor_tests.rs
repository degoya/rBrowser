use super::*;

/// The policy of the tests: two quick restarts, then give up.
fn quick() -> SupervisorConfig {
    SupervisorConfig {
        policy: RestartPolicy {
            max_crashes: 2,
            window: Duration::from_secs(60),
            first_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(20),
        },
        handshake_timeout: Duration::from_secs(5),
        shutdown_grace: Duration::from_millis(500),
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

fn failed_key(status: &HelperStatus) -> Option<&str> {
    match status {
        HelperStatus::Failed { reason_key, .. } => Some(reason_key),
        _ => None,
    }
}

#[test]
fn the_status_as_wsl_info_shows_it() {
    let running = HelperStatus::Running {
        pid: Some(7),
        helper_version: "0.3.0".into(),
        restarts: 1,
    };
    assert_eq!(
        serde_json::to_value(&running).expect("json"),
        serde_json::json!({"state": "running", "pid": 7, "helperVersion": "0.3.0", "restarts": 1})
    );
    assert_eq!(
        serde_json::to_value(HelperStatus::Stopped).expect("json"),
        serde_json::json!({"state": "stopped"})
    );
}

#[tokio::test]
async fn a_missing_program_is_given_up_as_a_failed_start() {
    let supervisor = Supervisor::start(
        || Command::new("/nonexistent/rbrowser-test/wsl.exe"),
        quick(),
    );
    let mut status = supervisor.subscribe();
    let failed = wait_for(&mut status, |s| failed_key(s).is_some()).await;
    assert_eq!(failed_key(&failed), Some("wsl.error.helperStart"));
    supervisor.shutdown().await;
}

#[cfg(unix)]
mod with_a_shell {
    use super::*;

    fn sh(script: &'static str) -> impl FnMut() -> Command {
        move || {
            let mut command = Command::new("sh");
            command.arg("-c").arg(script);
            command
        }
    }

    #[tokio::test]
    async fn a_helper_that_cannot_start_names_why() {
        let supervisor = Supervisor::start(
            sh(
                "echo 'execvpe(/mnt/c/x/rbrowser-wsl) failed: No such file or directory' >&2; exit 1",
            ),
            quick(),
        );
        let mut status = supervisor.subscribe();
        let failed = wait_for(&mut status, |s| failed_key(s).is_some()).await;
        let HelperStatus::Failed {
            reason_key,
            detail,
            restarts,
        } = failed
        else {
            unreachable!("waited for a failure");
        };
        assert_eq!(reason_key, "wsl.error.helperStart");
        assert!(detail.contains("execvpe"), "{detail}");
        assert_eq!(restarts, 2);
        supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn a_protocol_mismatch_is_not_retried() {
        let supervisor = Supervisor::start(
            sh(
                r#"read l; echo '{"type":"error","code":"protocol_mismatch","message":"x","protocol":99}'; sleep 30"#,
            ),
            quick(),
        );
        let mut status = supervisor.subscribe();
        let failed = wait_for(&mut status, |s| failed_key(s).is_some()).await;
        assert_eq!(failed_key(&failed), Some("wsl.error.protocolMismatch"));
        let HelperStatus::Failed {
            detail, restarts, ..
        } = &failed
        else {
            unreachable!("waited for a failure");
        };
        assert!(detail.contains("protocol 99"), "{detail}");
        assert_eq!(*restarts, 0);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(supervisor.status(), failed);
        supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn a_restart_after_giving_up_starts_again() {
        let supervisor = Supervisor::start(sh("exit 1"), quick());
        let mut status = supervisor.subscribe();
        wait_for(&mut status, |s| failed_key(s).is_some()).await;
        supervisor.restart();
        wait_for(&mut status, |s| {
            matches!(s, HelperStatus::Starting { restarts: 3 })
        })
        .await;
        supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn a_stop_during_the_handshake_ends_at_once() {
        let supervisor = Supervisor::start(sh("sleep 30"), quick());
        let mut status = supervisor.subscribe();
        wait_for(&mut status, |s| matches!(s, HelperStatus::Starting { .. })).await;
        tokio::time::timeout(Duration::from_secs(3), supervisor.shutdown())
            .await
            .expect("no waiting for the handshake timeout");
        assert_eq!(*status.borrow(), HelperStatus::Stopped);
    }

    #[tokio::test]
    async fn notices_before_the_handshake_are_skipped() {
        let supervisor = Supervisor::start(
            sh(
                r#"read l; n=$(echo "$l" | sed 's/.*"nonce":"\([0-9a-f]*\)".*/\1/'); echo 'wsl: a notice'; printf '{"type":"welcome","protocol":1,"helperVersion":"fake","nonce":"%s"}\n' "$n"; sleep 30"#,
            ),
            quick(),
        );
        let mut status = supervisor.subscribe();
        let running = wait_for(&mut status, |s| matches!(s, HelperStatus::Running { .. })).await;
        assert!(
            matches!(running, HelperStatus::Running { ref helper_version, .. } if helper_version == "fake")
        );
        supervisor.shutdown().await;
    }
}
