use std::sync::Arc;

use serde_json::Value;
use tokio::runtime::Handle;

use super::*;
use crate::reload::fake_host::{FakeHost, Page};

/// The CEF UI thread is outside the runtime, and CDP needs it to answer: a reload that waited
/// there would hang the window, so it is refused instead.
#[test]
fn a_reload_outside_the_runtime_is_refused_not_awaited() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("a runtime");
    let host = Arc::new(FakeHost::new(&[Page::default()]));
    let service = {
        let _inside = runtime.enter();
        HotReload::start(
            host.clone(),
            Handle::current(),
            None,
            ReloadOptions::default(),
            WslSettings::default(),
        )
    };
    assert!(Handle::try_current().is_err(), "outside the runtime");
    assert!(run_reload(&service, &Value::Null, None).is_err());
    assert!(host.reloaded().is_empty());
}

#[test]
fn a_reload_on_a_blocking_thread_of_the_runtime_runs() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("a runtime");
    let host = Arc::new(FakeHost::new(&[Page::default()]));
    let service = {
        let _inside = runtime.enter();
        HotReload::start(
            host.clone(),
            Handle::current(),
            None,
            ReloadOptions::default(),
            WslSettings::default(),
        )
    };
    let answer = runtime
        .block_on(runtime.spawn_blocking(move || run_reload(&service, &Value::Null, None)))
        .expect("the task ran");
    assert!(answer.is_ok());
    assert_eq!(host.reloaded().len(), 1);
}

/// `wsl.restart_helper` on a project that is not watched through the WSL helper says so.
#[test]
fn restart_helper_without_a_wsl_project_fails_clearly() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("a runtime");
    let host = Arc::new(FakeHost::new(&[Page::default()]));
    let service = {
        let _inside = runtime.enter();
        HotReload::start(
            host,
            Handle::current(),
            None,
            ReloadOptions::default(),
            WslSettings::default(),
        )
    };
    assert_eq!(
        run_restart_helper(service.restart_helper()),
        Err(CommandError::Failed(NO_HELPER_KEY.to_owned()))
    );
    assert_eq!(NO_HELPER_KEY, "wsl.error.noHelper");
}

/// RB-090-82: pausing hot reload from ≡ › Tools › Reload, the palette or its shortcut says what
/// it did - no control shows the watch state, and the row read as one that does nothing.
/// Resuming names the watch only when it is not the plain one ("Hot reload off" read as if the
/// toggle had switched hot reload off).
#[test]
fn the_pause_toggle_says_paused_or_resumed() {
    use super::classify::HotMode;
    let notice = |status: WatcherStatus, paused: bool| match pause_notice(&WatcherInfo::new(
        status,
        paused,
        HotMode::Auto,
        None,
    )) {
        rb_ipc::CoreToShell::Notification(notice) => {
            assert_eq!(notice.level, rb_ipc::NotificationLevel::Info);
            (notice.message_key, notice.args)
        }
        other => panic!("{other:?}"),
    };
    let none = std::collections::BTreeMap::new();
    assert_eq!(
        notice(WatcherStatus::Native { folders: 1 }, true),
        (PAUSED_KEY.to_owned(), none.clone())
    );
    assert_eq!(
        notice(WatcherStatus::Native { folders: 1 }, false),
        (RESUMED_KEY.to_owned(), none.clone())
    );
    assert_eq!(
        notice(
            WatcherStatus::Helper {
                distro: "debian".to_owned(),
                folders: 3
            },
            false
        ),
        (RESUMED_KEY.to_owned(), none)
    );
    let (key, args) = notice(WatcherStatus::Off, false);
    assert_eq!(key, RESUMED_STATE_KEY);
    assert_eq!(args["state"], rb_i18n::t("reload.status.off"));
}
