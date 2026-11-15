use std::path::Path;
use std::time::Duration;

use rb_wsl::helper::RestartPolicy;
use rb_wsl::watch::{Change, ChangeKind};

use super::*;

const WAIT: Duration = Duration::from_secs(5);

fn rules() -> WatchRules {
    WatchRules {
        debounce_ms: 50,
        ..WatchRules::default()
    }
}

async fn next_batch(inbox: &mut mpsc::UnboundedReceiver<SourceEvent>) -> Batch {
    loop {
        match tokio::time::timeout(WAIT, inbox.recv()).await {
            Ok(Some(SourceEvent::Batch(batch))) => return batch,
            Ok(Some(_)) => {}
            other => panic!("a batch, not {other:?}"),
        }
    }
}

#[test]
fn a_share_path_takes_the_helper_and_polling_wins_when_asked_for() {
    let share = Path::new(r"\\wsl.localhost\Ubuntu\home\u\app");
    assert_eq!(
        choose(share, false),
        SourceChoice::Helper(WslPath {
            distro: "Ubuntu".into(),
            linux_path: "/home/u/app".into(),
        })
    );
    assert_eq!(choose(share, true), SourceChoice::Polling);
    assert_eq!(
        choose(Path::new("/home/u/app"), false),
        SourceChoice::Native
    );
    assert_eq!(
        choose(Path::new(r"C:\work\app"), false),
        SourceChoice::Native
    );
}

#[tokio::test]
async fn the_native_watch_sends_one_batch_per_save() {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::create_dir_all(dir.path().join("node_modules/x")).expect("a folder");
    let (sink, mut inbox) = mpsc::unbounded_channel();
    let watch = start_native(dir.path(), &rules(), None, sink).expect("watching");
    assert_eq!(watch.folders(), 1);
    std::fs::write(dir.path().join("node_modules/x/a.js"), "x").expect("written");
    std::fs::write(dir.path().join("app.css"), "a{}").expect("written");
    let Batch::Changes(changes) = next_batch(&mut inbox).await else {
        panic!("changes");
    };
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].path, "app.css");
}

#[tokio::test]
async fn polling_sees_a_change_too() {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join("index.html"), "<p>").expect("written");
    let (sink, mut inbox) = mpsc::unbounded_channel();
    let _watch = start_native(dir.path(), &rules(), Some(Duration::from_millis(100)), sink)
        .expect("polling");
    // A poll compares modification times; make sure this write has a later one.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    std::fs::write(dir.path().join("index.html"), "<p>changed").expect("written");
    let Batch::Changes(changes) = next_batch(&mut inbox).await else {
        panic!("changes");
    };
    assert_eq!(changes, vec![Change::new("index.html", ChangeKind::Modify)]);
}

/// A dangling symlink (a WSL project's link to a container path) fails on every poll: it asks
/// for one rescan, not one per pass (the owner's 0.0.0 test reloaded every ~2 s).
#[cfg(unix)]
#[tokio::test]
async fn an_entry_that_fails_on_every_poll_rescans_once() {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::os::unix::fs::symlink(
        "/nonexistent/rbrowser/worker.js",
        dir.path().join("worker.js"),
    )
    .expect("a dangling link");
    let (sink, mut inbox) = mpsc::unbounded_channel();
    let _watch =
        start_native(dir.path(), &rules(), Some(Duration::from_millis(50)), sink).expect("polling");
    let mut rescans = 0;
    let end = tokio::time::Instant::now() + Duration::from_millis(1500);
    while let Ok(Some(event)) = tokio::time::timeout_at(end, inbox.recv()).await {
        if event == SourceEvent::Batch(Batch::Rescan) {
            rescans += 1;
        }
    }
    assert!(rescans <= 1, "{rescans} rescans in 30 polls");
}

#[tokio::test]
async fn a_bad_rule_or_a_missing_folder_does_not_start() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (sink, _inbox) = mpsc::unbounded_channel();
    let bad = WatchRules {
        ignore: vec!["[z-a]".into()],
        ..rules()
    };
    let error = start_native(dir.path(), &bad, None, sink.clone())
        .err()
        .expect("a bad rule");
    assert_eq!(error.key, "reload.error.badRule");
    let error = start_native(&dir.path().join("gone"), &rules(), None, sink)
        .err()
        .expect("no folder");
    assert_eq!(error.key, "reload.error.root");
}

#[tokio::test]
async fn a_helper_that_cannot_start_is_given_up() {
    let (sink, mut inbox) = mpsc::unbounded_channel();
    let spec = HelperSpec {
        distro: "Ubuntu".into(),
        factory: Box::new(|| Command::new("/nonexistent/rbrowser-wsl")),
        config: SupervisorConfig {
            policy: RestartPolicy {
                max_crashes: 1,
                window: Duration::from_secs(60),
                first_delay: Duration::from_millis(10),
                max_delay: Duration::from_millis(10),
            },
            handshake_timeout: Duration::from_secs(2),
            shutdown_grace: Duration::from_millis(100),
        },
    };
    let helper = start_helper(spec, rules(), sink, &Handle::current());
    let gave_up = loop {
        match tokio::time::timeout(WAIT, inbox.recv()).await {
            Ok(Some(SourceEvent::HelperGaveUp { reason_key, .. })) => break reason_key,
            Ok(Some(SourceEvent::Status(WatcherStatus::Starting))) => {}
            other => panic!("starting, then given up; not {other:?}"),
        }
    };
    assert!(gave_up.starts_with("wsl.error."), "{gave_up}");
    helper.shutdown().await;
}

#[test]
fn the_helper_messages_become_source_events() {
    assert_eq!(
        from_message(Message::Rescan, "Ubuntu"),
        Some(SourceEvent::Batch(Batch::Rescan))
    );
    assert_eq!(
        from_message(Message::Watching { folders: 7 }, "Ubuntu"),
        Some(SourceEvent::Status(WatcherStatus::Helper {
            distro: "Ubuntu".into(),
            folders: 7,
        }))
    );
    assert!(matches!(
        from_message(
            Message::Error {
                code: ErrorCode::WatchFailed,
                message: "limit".into(),
                protocol: None,
            },
            "Ubuntu"
        ),
        Some(SourceEvent::HelperGaveUp { reason_key, .. }) if reason_key == "reload.error.watchFailed"
    ));
    assert_eq!(from_message(Message::Pong, "Ubuntu"), None);
    assert_eq!(
        from_status(HelperStatus::Restarting {
            restarts: 1,
            delay_ms: 10
        }),
        Some(SourceEvent::Status(WatcherStatus::Starting))
    );
}

#[test]
fn the_packaged_helper_runs_through_wsl_exe() {
    let wsl = WslPath {
        distro: "Ubuntu".into(),
        linux_path: "/home/u/app".into(),
    };
    let settings = WslSettings {
        helper_path: Some("/opt/rbrowser-wsl".into()),
        ..WslSettings::default()
    };
    let mut spec = helper_spec(&wsl, &settings).expect("a spec");
    assert_eq!(spec.distro, "Ubuntu");
    let command = (spec.factory)();
    let args: Vec<String> = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args,
        [
            "-d",
            "Ubuntu",
            "-e",
            "/opt/rbrowser-wsl",
            "watch",
            "/home/u/app"
        ]
    );
    let other = WslSettings {
        distro: Some("Debian".into()),
        ..settings
    };
    assert_eq!(helper_spec(&wsl, &other).expect("a spec").distro, "Debian");
}
