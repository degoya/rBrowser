use std::sync::Arc;
use std::time::Duration;

use rb_engine::{ViewHost as _, ViewId};
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};
use crate::page_window::PageWindow;

fn core() -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    (ui, core)
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..400 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_entry_runs_its_command_off_the_calling_thread() {
    let (ui, core) = core();
    crate::panel_dock::tests::float(&core);
    run(
        &core,
        ViewId(1),
        &Action::Run {
            command: "panels.findings".to_owned(),
            args: json!({}),
        },
    );
    until(|| {
        ui.calls()
            .iter()
            .any(|call| matches!(call, UiCall::PageWindow(PageWindow::Panels, url) if url.ends_with("#/panels/findings")))
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_entry_is_named_in_the_status_bar() {
    let (_ui, core) = core();
    run(
        &core,
        ViewId(1),
        &Action::Run {
            command: "nothing.here".to_owned(),
            args: json!({}),
        },
    );
    until(|| !core.status_log().items().is_empty()).await;
    let message = &core.status_log().items()[0];
    assert_eq!(
        message.args.get("command").map(String::as_str),
        Some("nothing.here")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn do_not_sync_the_next_click_makes_the_view_active() {
    let (_ui, core) = core();
    assert_ne!(core.host().active_view(), Some(ViewId(3)));
    run(&core, ViewId(3), &Action::SkipNextClick);
    until(|| core.host().active_view() == Some(ViewId(3))).await;
}
