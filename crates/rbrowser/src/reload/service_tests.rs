use std::sync::Arc;
use std::time::Duration;

use rb_engine::{Target, ViewId};

use super::*;
use crate::reload::fake_host::{FakeHost, Page};
use crate::reload::mcp::ViewAction;

const WAIT: Duration = Duration::from_secs(5);

fn page() -> Page {
    Page {
        swaps: 1,
        ..Page::default()
    }
}

fn options() -> ReloadOptions {
    ReloadOptions {
        debounce_ms: 50,
        ..ReloadOptions::default()
    }
}

fn start(host: &Arc<FakeHost>, root: &std::path::Path, options: ReloadOptions) -> HotReload {
    HotReload::start(
        host.clone(),
        Handle::current(),
        Some(root.to_path_buf()),
        options,
        WslSettings::default(),
    )
}

async fn status_where(service: &HotReload, what: impl Fn(&WatcherStatus) -> bool) -> WatcherInfo {
    let mut info = service.subscribe();
    tokio::time::timeout(WAIT, info.wait_for(|info| what(&info.status)))
        .await
        .expect("the state came in time")
        .expect("the service is alive")
        .clone()
}

async fn until(what: impl Fn() -> bool) {
    tokio::time::timeout(WAIT, async {
        while !what() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("it happened in time");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_saved_stylesheet_is_swapped_and_a_script_reloads() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let host = Arc::new(FakeHost::new(&[page(), page()]));
    let service = start(&host, dir.path(), options());
    status_where(&service, |s| matches!(s, WatcherStatus::Native { .. })).await;

    std::fs::write(dir.path().join("app.css"), "a{}").expect("written");
    until(|| host.swaps_sent() == 2).await;
    assert!(host.reloaded().is_empty(), "CSS keeps the page");

    std::fs::write(dir.path().join("main.js"), "x").expect("written");
    until(|| host.reloaded().len() == 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_paused_or_off_watch_does_nothing() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let host = Arc::new(FakeHost::new(&[page()]));
    let service = start(&host, dir.path(), options());
    status_where(&service, |s| matches!(s, WatcherStatus::Native { .. })).await;
    assert!(service.toggle_pause());
    assert_eq!(service.info().label_key, "reload.status.paused");
    std::fs::write(dir.path().join("main.js"), "x").expect("written");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(host.reloaded().is_empty());
    assert!(!service.toggle_pause());

    let off = ReloadOptions {
        mode: HotMode::Off,
        ..options()
    };
    let service = start(&host, dir.path(), off);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(service.info().status, WatcherStatus::Off);
    assert_eq!(service.info().mode, "off");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn polling_when_asked_for_and_when_the_helper_gives_up() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let host = Arc::new(FakeHost::new(&[page()]));
    let polling = ReloadOptions {
        polling: true,
        ..options()
    };
    let service = start(&host, dir.path(), polling);
    let info = status_where(&service, |s| matches!(s, WatcherStatus::Polling { .. })).await;
    assert_eq!(
        info.status,
        WatcherStatus::Polling {
            folders: 1,
            reason_key: None
        }
    );
    assert_eq!(info.label_key, "reload.status.polling");

    // A helper that gave up (simulated): polling of the same root stands in, with the reason.
    let service = start(&host, dir.path(), options());
    status_where(&service, |s| matches!(s, WatcherStatus::Native { .. })).await;
    let generation = service.inner.state().generation;
    service
        .inner
        .on_event(
            generation,
            SourceEvent::HelperGaveUp {
                reason_key: "wsl.error.helperStart".into(),
                detail: "no helper".into(),
            },
        )
        .await;
    assert_eq!(
        service.info().status,
        WatcherStatus::Polling {
            folders: 1,
            reason_key: Some("wsl.error.helperStart".into())
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn events_of_an_older_generation_are_dropped() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let host = Arc::new(FakeHost::new(&[page()]));
    let service = start(&host, dir.path(), options());
    status_where(&service, |s| matches!(s, WatcherStatus::Native { .. })).await;
    let old = service.inner.state().generation - 1;
    service
        .inner
        .on_event(old, SourceEvent::Batch(Batch::Rescan))
        .await;
    service
        .inner
        .on_event(old, SourceEvent::Status(WatcherStatus::Starting))
        .await;
    assert!(host.reloaded().is_empty());
    assert!(matches!(
        service.info().status,
        WatcherStatus::Native { .. }
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_ignore_file_applies_at_once() {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::create_dir_all(dir.path().join(".rbrowser")).expect("a folder");
    std::fs::create_dir_all(dir.path().join("generated")).expect("a folder");
    let host = Arc::new(FakeHost::new(&[page()]));
    let service = start(&host, dir.path(), options());
    status_where(&service, |s| matches!(s, WatcherStatus::Native { .. })).await;
    let before = service.inner.state().generation;
    std::fs::write(dir.path().join(".rbrowser/ignore"), "generated/\n").expect("written");
    until(|| service.inner.state().generation > before).await;
    status_where(&service, |s| matches!(s, WatcherStatus::Native { .. })).await;
    assert!(host.reloaded().is_empty(), "the rules are no page change");
    std::fs::write(dir.path().join("generated/out.js"), "x").expect("written");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(host.reloaded().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_commands_reload_fully_or_swap_the_css() {
    let host = Arc::new(FakeHost::new(&[
        Page {
            hmr: true,
            ..page()
        },
        page(),
    ]));
    let service = HotReload::start(
        host.clone(),
        Handle::current(),
        None,
        options(),
        WslSettings::default(),
    );
    let css = service
        .reload(ReloadArgs {
            mode: ReloadKind::Css,
            ..ReloadArgs::default()
        })
        .await;
    assert_eq!(css.mode, ReloadKind::Css);
    assert!(
        css.views
            .iter()
            .all(|v| v.action == ViewAction::Swapped { count: 1 })
    );
    assert!(host.reloaded().is_empty());
    let full = service
        .reload(ReloadArgs {
            target: Target::View(ViewId(1)),
            ignore_cache: true,
            ..ReloadArgs::default()
        })
        .await;
    assert_eq!(full.views.len(), 1);
    assert_eq!(full.views[0].action, ViewAction::Reloaded, "HMR or not");
    assert_eq!(service.info().status, WatcherStatus::Off, "no project");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_interval_reloads_when_due_and_waits_while_someone_types() {
    let typing = Arc::new(FakeHost::new(&[Page {
        typing: true,
        ..page()
    }]));
    let idle = Arc::new(FakeHost::new(&[page()]));
    let every_10s = ReloadOptions {
        interval_sec: 10,
        ..options()
    };
    for (host, reloads) in [(typing, 0), (idle, 1)] {
        let service = HotReload::start(
            host.clone(),
            Handle::current(),
            None,
            every_10s.clone(),
            WslSettings::default(),
        );
        assert_eq!(service.info().interval_sec, Some(10));
        // Due now: the last reload was a period ago.
        let ago = Instant::now()
            .checked_sub(Duration::from_secs(11))
            .expect("the clock is past 11 s");
        service.inner.state().interval = Some(IntervalReload::new(
            Duration::from_secs(10),
            Duration::from_secs(5),
            ago,
        ));
        service.inner.interval_step().await;
        // The service's own interval task may have taken the due step first: exactly one of
        // the two reloads, never both.
        until(|| host.reloaded().len() >= reloads).await;
        service.inner.interval_step().await;
        assert_eq!(host.reloaded().len(), reloads);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_environment_has_its_own_interval() {
    let host = Arc::new(FakeHost::new(&[page()]));
    let mut staging = options();
    staging.environment_intervals.insert("staging".into(), 10);
    let service = HotReload::start(
        host.clone(),
        Handle::current(),
        None,
        staging,
        WslSettings::default(),
    );
    assert_eq!(service.info().interval_sec, None);
    service.set_environment(Some("staging".into()));
    assert_eq!(service.info().interval_sec, Some(10));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unchanged_settings_keep_the_watch_running() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let host = Arc::new(FakeHost::new(&[page()]));
    let service = start(&host, dir.path(), options());
    status_where(&service, |s| matches!(s, WatcherStatus::Native { .. })).await;
    let before = service.inner.state().generation;
    // Another user setting was written: root, options and wsl are the same.
    service.apply(
        Some(dir.path().to_path_buf()),
        options(),
        WslSettings::default(),
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(service.inner.state().generation, before);
    let slower = ReloadOptions {
        debounce_ms: 80,
        ..options()
    };
    service.apply(
        Some(dir.path().to_path_buf()),
        slower,
        WslSettings::default(),
    );
    until(|| service.inner.state().generation > before).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stale_generation_installs_no_source() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let host = Arc::new(FakeHost::new(&[page()]));
    let service = start(&host, dir.path(), options());
    status_where(&service, |s| matches!(s, WatcherStatus::Native { .. })).await;
    let stale = service.inner.state().generation - 1;
    assert!(
        !service
            .inner
            .install(stale, Source::None, WatcherStatus::Starting)
    );
    assert!(matches!(
        service.inner.state().source,
        Source::Native { .. }
    ));
    assert!(matches!(
        service.info().status,
        WatcherStatus::Native { .. }
    ));
}

/// `wsl.restart_helper`: a WSL project's helper starts again (also after it was given up and
/// polling stood in), as a new generation; a project that is not watched through the helper
/// answers `false` and keeps its watch.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_helper_starts_the_helper_of_a_wsl_project_again() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let host = Arc::new(FakeHost::new(&[page()]));
    let specs = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&specs);
    let service = HotReload::start_with(
        host.clone(),
        Handle::current(),
        Some(PathBuf::from(r"\\wsl.localhost\Ubuntu\home\u\app")),
        options(),
        WslSettings::default(),
        Box::new(move |_, _| {
            counted.fetch_add(1, Ordering::SeqCst);
            Err(StartError {
                key: "wsl.error.helperStart".into(),
                detail: "no helper in the test".into(),
            })
        }),
    );
    until(|| specs.load(Ordering::SeqCst) == 1).await;
    let before = service.inner.state().generation;
    assert!(service.restart_helper());
    until(|| specs.load(Ordering::SeqCst) == 2).await;
    assert!(service.inner.state().generation > before);

    let dir = tempfile::tempdir().expect("a temp dir");
    let local = start(&host, dir.path(), options());
    status_where(&local, |s| matches!(s, WatcherStatus::Native { .. })).await;
    let before = local.inner.state().generation;
    assert!(!local.restart_helper());
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(local.inner.state().generation, before);
}
