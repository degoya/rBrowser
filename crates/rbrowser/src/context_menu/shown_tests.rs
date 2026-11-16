use std::sync::Arc;
use std::time::Duration;

use rb_engine::ViewId;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::context_menu::model::{Entry, FIRST_ID, MenuState, PluginCommand, PluginPanel};
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

fn state(view: ViewId) -> MenuState {
    MenuState {
        view,
        point: Some((1, 2)),
        menu_enabled: true,
        attached: true,
        detached: false,
        devtools_open: false,
        open_on_inspect: true,
        emulation: None,
        recording: None,
        checks_busy: false,
        checks: vec![PluginCommand {
            command: "plugin.seo.check".to_owned(),
            title: "SEO check".to_owned(),
        }],
        panels: vec![PluginPanel {
            plugin: "network-panel".to_owned(),
            panel: "network".to_owned(),
            title: "Network".to_owned(),
        }],
        known: ["view.remove"].into_iter().map(str::to_owned).collect(),
        footer_visible: true,
        forced: crate::force_state::ForcedMarks::default(),
    }
}

fn entries(items: &[Item]) -> Vec<&Entry> {
    let mut found = Vec::new();
    for item in items {
        match item {
            Item::Entry(entry) => found.push(entry),
            Item::Separator => {}
            Item::Menu { items, .. } => found.extend(entries(items)),
        }
    }
    found
}

fn id_of(items: &[Item], command: &str) -> i32 {
    entries(items)
        .into_iter()
        .find(|entry| matches!(&entry.action, Action::Run { command: id, .. } if id == command))
        .unwrap_or_else(|| panic!("no entry runs {command}"))
        .id
}

/// The defect of RB-080-45: CEF asks the client for the handler once per callback, and the
/// client makes one per call - the handler that gets the choice never saw the menu open. The
/// client holds the view's menu and every handler a clone of it: every entry of the block one
/// handler showed is found by another.
#[tokio::test]
async fn a_second_handler_of_the_view_finds_every_entry_the_first_one_showed() {
    let (_ui, core) = core();
    let view = ViewId(41);
    let client = ViewMenu::for_view(view);
    let items = client.clone().opened(&core, Some((10, 20)));
    let shown = entries(&items);
    assert!(
        shown.len() > 10,
        "the whole block, submenus included: {}",
        shown.len()
    );
    for entry in shown {
        assert_eq!(
            client.clone().action(entry.id),
            Some(entry.action.clone()),
            "entry {} ({:?})",
            entry.id,
            entry.label
        );
    }
}

/// The same for the entries a window without plugins and with one view never shows: a
/// plugin's check and panel, "for all views", pop out and remove view.
#[test]
fn plugin_all_views_pop_out_and_remove_entries_are_found_by_a_second_handler() {
    let view = ViewId(42);
    let items = build(&state(view));
    let client = ViewMenu::for_view(view);
    client.clone().remember(items.clone());
    for command in [
        "devtools.inspect_at",
        "capture.views",
        "capture.all_viewport",
        "plugin.seo.check",
        rb_plugin_host::PANELS_COMMAND,
        crate::pop_out::POP_OUT,
        "view.remove",
    ] {
        let id = id_of(&items, command);
        let action = client.clone().action(id);
        assert!(
            matches!(&action, Some(Action::Run { command: found, .. }) if found == command),
            "{command}: {action:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_choice_made_on_a_second_handler_runs_the_entrys_command() {
    let (ui, core) = core();
    crate::panel_dock::tests::float(&core);
    let view = ViewId(43);
    let client = ViewMenu::for_view(view);
    let items = client.clone().opened(&core, None);
    let id = id_of(&items, "panels.findings");
    assert!(client.clone().chosen(&core, id), "handled");
    until(|| {
        ui.calls()
            .iter()
            .any(|call| matches!(call, UiCall::PageWindow(PageWindow::Panels, url) if url.ends_with("#/panels/findings")))
    })
    .await;
}

/// A screenshot that cannot run (no CDP session in this window) says so in the status bar.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_chosen_entry_that_fails_is_named_in_the_status_bar() {
    let (_ui, core) = core();
    let view = ViewId(44);
    let client = ViewMenu::for_view(view);
    let items = client.clone().opened(&core, Some((5, 5)));
    let id = id_of(&items, "capture.full_page");
    assert!(client.clone().chosen(&core, id));
    until(|| !core.status_log().items().is_empty()).await;
    let message = &core.status_log().items()[0];
    assert_eq!(message.level, rb_ipc::NotificationLevel::Error);
    assert!(
        message.message_key == crate::capture::FAILED_KEY
            || message.args.get("command").map(String::as_str) == Some("capture.full_page"),
        "{message:?}"
    );
}

/// "Screenshot > This view" from the menu: the file lands in the project's screenshot folder
/// and the status bar names the folder.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_screenshot_from_the_menu_writes_the_file_and_reports_it() {
    use crate::devtools::session::tests::{attach_views, setup};
    use base64::Engine as _;

    let project = tempfile::tempdir().expect("temp dir");
    let setup = setup();
    setup
        .core
        .host()
        .open_config(default_config(), Some(project.path().to_path_buf()))
        .expect("opens");
    attach_views(&setup).await;
    let image = rb_visual::Rgba::new(4, 4, [0, 0, 200, 255]).expect("small");
    let png = rb_visual::encode_png(&image).expect("encoded");
    setup.fake.answer_with(
        "Page.captureScreenshot",
        serde_json::json!({ "data": base64::engine::general_purpose::STANDARD.encode(png) }),
    );
    setup.fake.answer_with(
        "Page.getLayoutMetrics",
        serde_json::json!({ "cssVisualViewport": { "pageX": 0, "pageY": 0 } }),
    );
    let view = ViewId(2);
    let client = ViewMenu::for_view(view);
    let items = client.clone().opened(&setup.core, Some((5, 5)));
    let id = id_of(&items, "capture.views");
    assert!(client.clone().chosen(&setup.core, id));
    let log = setup.core.status_log();
    until(|| {
        log.items().iter().any(|message| {
            message.message_key == crate::capture::SAVED_KEY
                || message.level == rb_ipc::NotificationLevel::Error
        })
    })
    .await;
    let items = log.items();
    let saved = items
        .iter()
        .find(|message| message.message_key == crate::capture::SAVED_KEY)
        .unwrap_or_else(|| panic!("the screenshot failed: {items:?}"));
    assert_eq!(saved.args.get("count").map(String::as_str), Some("1"));
    let folder = std::path::PathBuf::from(saved.args.get("folder").expect("the folder"));
    assert!(folder.starts_with(project.path()), "{folder:?}");
    let files: Vec<_> = std::fs::read_dir(&folder)
        .expect("the folder exists")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
}

/// Only the ids this menu handed out are its own: Chromium's, a submenu's and every other id
/// of CEF's user range are left to the default handling. An id it did hand out that the last
/// block no longer has is handled, and the status bar says the entry is gone.
#[tokio::test]
async fn only_ids_the_menu_handed_out_are_claimed() {
    let (_ui, core) = core();
    let client = ViewMenu::for_view(ViewId(45));
    assert!(!client.clone().chosen(&core, FIRST_ID), "nothing shown yet");
    let items = client.clone().opened(&core, None);
    let last = entries(&items)
        .into_iter()
        .map(|entry| entry.id)
        .max()
        .expect("entries");
    assert!(!client.clone().chosen(&core, 110), "MENU_ID_BACK");
    assert!(
        !client
            .clone()
            .chosen(&core, crate::context_menu::model::FIRST_MENU_ID),
        "a submenu's id is never chosen"
    );
    assert!(
        !client.clone().chosen(&core, last + 1),
        "in the block's range, but never handed out"
    );
    assert!(!client.clone().chosen(&core, 28_500), "MENU_ID_USER_LAST");
    assert!(core.status_log().items().is_empty());
    // A later, shorter block: the ids the longer one had are still this menu's.
    client.clone().remember(vec![Item::Separator]);
    assert!(client.clone().chosen(&core, last), "handled");
    assert!(!client.clone().chosen(&core, last + 1));
    let messages = core.status_log().items();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert_eq!(messages[0].message_key, ENTRY_GONE_KEY);
    assert_eq!(messages[0].level, rb_ipc::NotificationLevel::Warning);
}

/// The block belongs to the view's client: a later client - a view with the same id again,
/// or the view in another window - starts with none, and dropping the client drops the block.
#[test]
fn the_block_goes_with_the_views_client() {
    let view = ViewId(46);
    let client = ViewMenu::for_view(view);
    let handler = client.clone();
    let items = build(&state(view));
    handler.remember(items.clone());
    let id = id_of(&items, "view.remove");
    assert!(client.action(id).is_some());
    assert_eq!(ViewMenu::for_view(view).action(id), None, "a new client");
    let weak = std::sync::Arc::downgrade(&client.shown);
    drop((client, handler));
    assert_eq!(weak.strong_count(), 0, "nothing outlives the client");
}
