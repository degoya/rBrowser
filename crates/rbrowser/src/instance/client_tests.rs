use std::sync::{Arc, Mutex};

use super::*;
use crate::instance::server::{Handler, start};

fn recording() -> (Handler, Arc<Mutex<Vec<Request>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let handler: Handler = Arc::new(move |request| {
        log.lock().expect("lock").push(request);
        Ok(None)
    });
    (handler, seen)
}

#[test]
fn a_second_call_for_the_same_project_reaches_the_running_instance() {
    let local = tempfile::tempdir().expect("temp dir");
    let root = local.path().join("site");
    let (handler, seen) = recording();
    start(local.path(), &root, handler).expect("listens");
    assert!(file::file_path(local.path(), &root).is_file());

    let open = Request::Open {
        urls: vec!["localhost:5173/a".to_owned()],
        background: true,
    };
    assert_eq!(hand_over(local.path(), &root, open), Outcome::Handed);
    let reload = Request::Reload {
        ignore_cache: false,
    };
    assert_eq!(
        hand_over(local.path(), &root, reload.clone()),
        Outcome::Handed
    );
    assert_eq!(
        *seen.lock().expect("lock"),
        vec![
            Request::Open {
                urls: vec!["http://localhost:5173/a".to_owned()],
                background: true,
            },
            reload,
        ]
    );
}

#[test]
fn another_project_has_no_running_instance() {
    let local = tempfile::tempdir().expect("temp dir");
    let (handler, _) = recording();
    start(local.path(), &local.path().join("site"), handler).expect("listens");
    let other = local.path().join("other");
    let reload = Request::Reload {
        ignore_cache: false,
    };
    assert_eq!(hand_over(local.path(), &other, reload), Outcome::NotRunning);
}

#[test]
fn a_file_left_by_a_crash_is_no_running_instance() {
    let local = tempfile::tempdir().expect("temp dir");
    let root = local.path().join("site");
    let dir = file::instances_dir(local.path());
    let stale = file::InstanceFile {
        version: file::FILE_VERSION,
        pid: 1,
        endpoint: channel::endpoint_for(&dir, &file::key(&root)),
        nonce: protocol::new_nonce().expect("random"),
        project: root.to_string_lossy().into_owned(),
    };
    file::write(&file::file_path(local.path(), &root), &stale).expect("writes");
    let reload = Request::Reload {
        ignore_cache: false,
    };
    assert_eq!(hand_over(local.path(), &root, reload), Outcome::NotRunning);
}

#[test]
fn a_call_with_the_wrong_nonce_is_refused() {
    let local = tempfile::tempdir().expect("temp dir");
    let root = local.path().join("site");
    let (handler, seen) = recording();
    start(local.path(), &root, handler).expect("listens");
    let path = file::file_path(local.path(), &root);
    let mut forged = file::read(&path, local.path()).expect("an instance file");
    forged.nonce = protocol::new_nonce().expect("random");
    file::write(&path, &forged).expect("writes");
    let reload = Request::Reload {
        ignore_cache: false,
    };
    assert_eq!(
        hand_over(local.path(), &root, reload),
        Outcome::Refused(Refusal::BadNonce, None)
    );
    assert!(seen.lock().expect("lock").is_empty());
}

#[test]
fn a_screenshot_comes_back_with_its_answer_or_its_reason() {
    let local = tempfile::tempdir().expect("temp dir");
    let root = local.path().join("site");
    let handler: Handler = Arc::new(|request: Request| match request {
        Request::Screenshot { views, .. } if views.is_empty() => {
            Ok(Some(serde_json::json!({ "shots": [], "folder": "/p" })))
        }
        _ => Err("no view shows tablet".to_owned()),
    });
    start(local.path(), &root, handler).expect("listens");
    let shot = |views: Vec<String>| Request::Screenshot {
        args: crate::capture::ScreenshotArgs::default(),
        views,
        out: None,
    };
    assert_eq!(
        hand_over(local.path(), &root, shot(Vec::new())),
        Outcome::Answered(serde_json::json!({ "shots": [], "folder": "/p" }))
    );
    assert_eq!(
        hand_over(local.path(), &root, shot(vec!["tablet".to_owned()])),
        Outcome::Refused(Refusal::Failed, Some("no view shows tablet".to_owned()))
    );
}

#[test]
fn a_screenshot_is_waited_for_beyond_its_delay() {
    let delayed = Request::Screenshot {
        args: crate::capture::ScreenshotArgs {
            delay_ms: Some(60_000),
            ..crate::capture::ScreenshotArgs::default()
        },
        views: Vec::new(),
        out: None,
    };
    assert!(reply_timeout(&delayed) > Duration::from_secs(60) + protocol::SCREENSHOT_MARGIN);
    let reload = Request::Reload {
        ignore_cache: false,
    };
    assert_eq!(reply_timeout(&reload), REPLY_TIMEOUT);
    let check = Request::Check {
        args: crate::checks::export::check::CheckRequest::default(),
    };
    assert!(reply_timeout(&check) > crate::checks::export::check::CHECK_BUDGET);
}

#[test]
fn a_long_screenshot_does_not_hold_up_other_calls() {
    let local = tempfile::tempdir().expect("temp dir");
    let root = local.path().join("site");
    let (release, wait) = std::sync::mpsc::channel::<()>();
    let wait = Mutex::new(wait);
    let handler: Handler = Arc::new(move |request: Request| match request {
        Request::Screenshot { .. } => {
            let _ = wait
                .lock()
                .expect("lock")
                .recv_timeout(Duration::from_secs(30));
            Ok(Some(serde_json::json!({ "shots": [] })))
        }
        _ => Ok(None),
    });
    start(local.path(), &root, handler).expect("listens");
    let (shot_local, shot_root) = (local.path().to_path_buf(), root.clone());
    let shot = std::thread::spawn(move || {
        let request = Request::Screenshot {
            args: crate::capture::ScreenshotArgs::default(),
            views: Vec::new(),
            out: None,
        };
        hand_over(&shot_local, &shot_root, request)
    });
    std::thread::sleep(Duration::from_millis(200));
    let reload = Request::Reload {
        ignore_cache: false,
    };
    assert_eq!(hand_over(local.path(), &root, reload), Outcome::Handed);
    release.send(()).expect("sends");
    assert!(matches!(shot.join().expect("joins"), Outcome::Answered(_)));
}
