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
        run_restart_helper(&service),
        Err(CommandError::Failed(NO_HELPER_KEY.to_owned()))
    );
    assert_eq!(NO_HELPER_KEY, "wsl.error.noHelper");
}
