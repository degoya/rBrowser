use std::time::{Duration, Instant};

use rb_commands::keymap::{EVENTFLAG_CONTROL_DOWN, EVENTFLAG_SHIFT_DOWN};
use rb_commands::{CommandDef, Focus, Group, McpCoverage};
use serde_json::json;

use rb_ipc::IpcErrorCode;

use super::KeyDown;
use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};
use crate::page_window::PageWindow;

fn core_with(setup: CoreSetup) -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui.clone(), Handle::current(), setup);
    (ui, core)
}

fn core() -> (Arc<RecordingUi>, Arc<Core>) {
    core_with(CoreSetup::default())
}

/// The messages sent to the shell, as JSON.
fn sent(ui: &RecordingUi) -> Vec<Value> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) => {
                let start = script.find("detail:")? + "detail:".len();
                let end = script.rfind("}));")?;
                serde_json::from_str(&script[start..end]).ok()
            }
            _ => None,
        })
        .collect()
}

fn key(code: u32, character: char) -> KeyDown {
    KeyDown {
        flags: 0,
        windows_key_code: code,
        character: Some(character),
        unmodified_character: Some(character.to_ascii_lowercase()),
    }
}

fn leader() -> KeyDown {
    KeyDown {
        flags: EVENTFLAG_CONTROL_DOWN | EVENTFLAG_SHIFT_DOWN,
        windows_key_code: 0x20,
        character: Some(' '),
        unmodified_character: Some(' '),
    }
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

#[tokio::test]
async fn every_command_registers_without_a_conflict() {
    let (_ui, core) = core();
    let mut registry = CommandRegistry::new();
    let refused = register_all(&mut registry, &core, None);
    assert!(refused.is_empty(), "{refused:?}");
    let CoreToShell::Commands {
        leader, commands, ..
    } = core.commands()
    else {
        panic!("commands");
    };
    assert_eq!(leader, "Ctrl+Shift+Space");
    let ids: Vec<&str> = commands.iter().map(|info| info.id.as_str()).collect();
    for id in [
        "view.back",
        "view.zoom_fit_all",
        "diagnostics.create",
        "plugins.list",
        "app.command_palette",
    ] {
        assert!(ids.contains(&id), "{id} missing");
    }
}

#[tokio::test]
async fn the_leader_shows_which_key_and_a_chord_runs_its_command() {
    let (ui, core) = core();
    let now = Instant::now();
    assert!(core.on_key_down(Focus::View, leader(), now));
    assert!(core.on_key_down(Focus::View, key(0xBC, ','), now));
    let shown = sent(&ui);
    assert_eq!(shown[0]["type"], "whichKey");
    assert_eq!(shown[1]["type"], "whichKey");
    assert_eq!(shown[1]["typed"], "Ctrl+Shift+Space ,");
    // `, ,` opens the settings window (RB-010-08).
    assert!(core.on_key_down(Focus::View, key(0xBC, ','), now));
    until(|| {
        ui.calls().iter().any(|call| {
            matches!(call, UiCall::SettingsWindow(url) if url == "rbrowser://ui/settings.html#/settings")
        })
    })
    .await;
    assert!(sent(&ui).iter().any(|m| m["type"] == "whichKeyHide"));
}

#[tokio::test]
async fn keys_that_are_not_rbrowsers_pass_through() {
    let (ui, core) = core();
    assert!(!core.on_key_down(Focus::View, key(0x41, 'a'), Instant::now()));
    assert!(!core.on_key_down(Focus::External, leader(), Instant::now()));
    assert!(sent(&ui).is_empty());
}

#[tokio::test]
async fn a_pending_leader_waits_until_a_key_unless_a_timeout_is_set() {
    let (ui, core) = core();
    let start = Instant::now();
    assert!(core.on_key_down(Focus::Shell, leader(), start));
    core.on_tick(start + Duration::from_secs(3600));
    assert_eq!(
        sent(&ui).last().expect("sent")["type"],
        "whichKey",
        "by default the overlay stays (RB-080-18)"
    );
    let mut user = core.user_settings();
    user.shortcuts.leader_timeout_ms = 4000;
    core.replace_user_settings(user);
    core.on_tick(start + Duration::from_secs(3605));
    assert_eq!(sent(&ui).last().expect("sent")["type"], "whichKeyHide");
}

#[tokio::test]
async fn the_editor_sets_checks_and_resets_a_binding_and_saves_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("rBrowser").join("settings.json");
    let (_ui, core) = core_with(CoreSetup {
        user_path: Some(path.clone()),
        ..CoreSetup::default()
    });
    let check = core
        .check_binding("view.back", "Ctrl+Shift+Space R R")
        .expect("checked");
    assert_eq!(
        check,
        CoreToShell::BindingCheck(rb_ipc::BindingCheck {
            ok: false,
            reason_key: Some("shortcuts.rejected.conflict".to_owned()),
            conflict: Some("reload.all".to_owned()),
        })
    );
    core.set_bindings("view.back", &["Ctrl+Shift+Space V J".to_owned()])
        .expect("set");
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("written")).expect("json");
    assert_eq!(
        saved["shortcuts"]["bindings"]["view.back"],
        json!(["Ctrl+Shift+Space V J"])
    );
    let exported = core.export_bindings();
    let CoreToShell::BindingsExport { json: file } = exported else {
        panic!("export");
    };
    core.reset_binding(None).expect("reset");
    assert!(core.user_settings().shortcuts.bindings.is_empty());
    core.import_bindings(&file).expect("imported");
    assert_eq!(core.user_settings().shortcuts.bindings.len(), 1);
    assert_eq!(
        core.set_bindings("view.back", &["Ctrl+T".to_owned()])
            .expect_err("reserved")
            .0,
        IpcErrorCode::Refused
    );
}

#[tokio::test]
async fn binding_notices_wait_for_the_shell() {
    let mut user = UserSettings::default();
    user.shortcuts
        .bindings
        .insert("view.teleport".to_owned(), vec!["Ctrl+Shift+F4".to_owned()]);
    let (ui, core) = core_with(CoreSetup {
        user,
        ..CoreSetup::default()
    });
    assert!(sent(&ui).is_empty());
    core.shell_ready();
    let shown = sent(&ui);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0]["messageKey"], "shortcuts.notice.unknown_command");
    assert_eq!(shown[0]["args"]["command"], "view.teleport");
}

#[tokio::test]
async fn an_old_cef_build_is_shown_once_the_shell_is_ready() {
    let old = rb_diag::CefBuild {
        version: "156.0.26+g8ea0c73+chromium-156.0.8078.88".to_owned(),
        date: "2000-01-01".to_owned(),
    };
    let (ui, core) = core_with(CoreSetup {
        cef_build: Some(old.clone()),
        ..CoreSetup::default()
    });
    assert!(sent(&ui).is_empty());
    core.shell_ready();
    let shown = sent(&ui);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0]["type"], "notification");
    assert_eq!(shown[0]["messageKey"], "diagnostics.cefAge.notice");
    assert_eq!(shown[0]["settingsKey"], "updates.cefAgeWarning");

    let mut user = UserSettings::default();
    user.updates.cef_age_warning = false;
    let (ui, core) = core_with(CoreSetup {
        user,
        cef_build: Some(old),
        ..CoreSetup::default()
    });
    core.shell_ready();
    assert!(sent(&ui).is_empty());
}

/// A command that signals `entered` when it starts and runs until `release` sends.
fn blocking_command(
    core: &Core,
) -> (
    CommandId,
    std::sync::mpsc::Receiver<()>,
    std::sync::mpsc::Sender<()>,
) {
    let id = CommandId::new("view.test_block").expect("id");
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let (entered_tx, release_rx) = (Mutex::new(entered_tx), Mutex::new(release_rx));
    let def = CommandDef {
        id: id.clone(),
        title_key: "commands.view.test_block".to_owned(),
        group: Group::View,
        default_bindings: vec!["Ctrl+Shift+Space V .".parse().expect("binding")],
        mcp: McpCoverage::Omitted("test".to_owned()),
    };
    let handler: rb_commands::Handler = Arc::new(move |_| {
        let _ = lock(&entered_tx).send(());
        let _ = lock(&release_rx).recv();
        Ok(Value::Null)
    });
    lock(&core.registry)
        .register(def, handler)
        .expect("registers");
    (id, entered_rx, release_tx)
}

#[tokio::test]
async fn a_running_command_leaves_the_registry_to_the_keys() {
    let (_ui, core) = core();
    let (id, entered, release) = blocking_command(&core);
    core.spawn_command(id);
    entered
        .recv_timeout(Duration::from_secs(5))
        .expect("the command runs");
    assert!(
        core.registry.try_lock().is_ok(),
        "a key press must not wait for a running command"
    );
    release.send(()).expect("released");
}

#[tokio::test]
async fn a_command_from_the_shell_answers_when_done_without_holding_the_caller() {
    let (_ui, core) = core();
    let (id, entered, release) = blocking_command(&core);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    core.run_command_async(id.as_str(), Value::Null, move |result| {
        let _ = done_tx.send(result);
    });
    entered
        .recv_timeout(Duration::from_secs(5))
        .expect("the command runs");
    assert!(
        done_rx.try_recv().is_err(),
        "the caller is back before the command ends"
    );
    release.send(()).expect("released");
    let result = done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("answered");
    assert_eq!(result.expect("ran"), Value::Null);
}

#[tokio::test]
async fn the_editor_keeps_the_entries_the_registry_did_not_take() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    let mut user = UserSettings::default();
    for (id, binding) in [
        ("plugin.later.run", "Ctrl+Shift+F4"),
        ("view.back", "Ctrl+W"),
    ] {
        user.shortcuts
            .bindings
            .insert(id.to_owned(), vec![binding.to_owned()]);
    }
    let (_ui, core) = core_with(CoreSetup {
        user,
        user_path: Some(path.clone()),
        ..CoreSetup::default()
    });
    core.set_bindings("view.forward", &["Ctrl+Shift+Space V J".to_owned()])
        .expect("set");
    let saved = |key: &str| {
        let file: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("written")).expect("json");
        file["shortcuts"]["bindings"][key].clone()
    };
    assert_eq!(saved("plugin.later.run"), json!(["Ctrl+Shift+F4"]));
    assert_eq!(
        saved("view.back"),
        json!(["Ctrl+W"]),
        "refused, but the user's"
    );
    assert_eq!(saved("view.forward"), json!(["Ctrl+Shift+Space V J"]));
    core.reset_binding(Some("view.back")).expect("reset");
    assert_eq!(saved("view.back"), Value::Null, "a reset drops it");
    core.reset_binding(None).expect("reset all");
    assert_eq!(
        saved("plugin.later.run"),
        json!(["Ctrl+Shift+F4"]),
        "a plugin not loaded yet keeps its entry"
    );
}

#[tokio::test]
async fn nothing_is_written_over_a_settings_file_that_did_not_load() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    std::fs::write(&path, "{ broken").expect("write");
    let (ui, core) = core_with(CoreSetup {
        user_path: Some(path.clone()),
        user_load_failed: true,
        ..CoreSetup::default()
    });
    core.shell_ready();
    core.set_bindings("view.back", &["Ctrl+Shift+Space V J".to_owned()])
        .expect("applies for this session");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "{ broken");
    assert!(
        sent(&ui)
            .iter()
            .any(|message| message["messageKey"] == "shortcuts.notice.not_saved")
    );
}

#[test]
fn dropped_plugin_defaults_are_kept_per_command() {
    use rb_commands::registry::{NOTICE_BINDING_DROPPED, Notice};
    let notice = |key: &str, command: &str, binding: Option<&str>| Notice {
        message_key: key.to_owned(),
        command: command.to_owned(),
        binding: binding.map(str::to_owned),
        reason_key: None,
    };
    let dropped = super::bindings::dropped_defaults(&[
        notice(NOTICE_BINDING_DROPPED, "plugin.a.run", Some("Ctrl+W")),
        notice(NOTICE_BINDING_DROPPED, "plugin.a.run", Some("Ctrl+T")),
        notice("shortcuts.notice.other", "plugin.b.run", Some("Ctrl+Q")),
        notice(NOTICE_BINDING_DROPPED, "plugin.c.run", None),
    ]);
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped["plugin.a.run"], vec!["Ctrl+W", "Ctrl+T"]);
}

/// The palette and the findings panel open windows of their own; CEF runs a shortcut's command
/// on a blocking thread, a query's on the UI thread outside the runtime - the start path takes no
/// runtime context (the runtime rule).
#[test]
fn the_palette_and_the_findings_panel_open_their_windows_outside_the_runtime() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("a runtime");
    let (ui, core) = {
        let _inside = runtime.enter();
        core()
    };
    assert!(Handle::try_current().is_err(), "outside the runtime");
    crate::panel_dock::tests::float(&core);
    core.run_command("app.command_palette", Value::Null)
        .expect("opens");
    core.run_command("panels.findings", Value::Null)
        .expect("opens");
    let calls = ui.calls();
    let palette = PageWindow::Palette;
    assert!(calls.contains(&UiCall::PageWindow(palette, palette.url())));
    // The findings in the panels page (RB-080-69).
    assert!(calls.contains(&UiCall::PageWindow(
        PageWindow::Panels,
        "rbrowser://ui/settings.html#/panels/findings".to_owned()
    )));
    assert!(
        !calls.contains(&UiCall::FocusShell),
        "the shell strip keeps its 44 px and never takes the palette"
    );
}

/// RB-080-60 review: an edited binding reaches the shell as the registry's commands, so a
/// tooltip that names a shortcut follows the edit without asking.
#[tokio::test]
async fn an_edited_binding_is_sent_to_the_shell() {
    let (ui, core) = core();
    core.set_bindings("view.back", &["Ctrl+Shift+Space V J".to_owned()])
        .expect("set");
    let commands = sent(&ui)
        .into_iter()
        .rfind(|message| message["type"] == "commands")
        .expect("a commands event");
    let back = commands["commands"]
        .as_array()
        .and_then(|all| all.iter().find(|command| command["id"] == "view.back"))
        .expect("view.back");
    assert_eq!(back["bindings"], json!(["Ctrl+Shift+Space V J"]));
}
