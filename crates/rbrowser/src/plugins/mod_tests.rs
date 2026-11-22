use std::time::Duration;

use rb_commands::{CommandDef, CommandId, Group, McpCoverage};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
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

#[tokio::test(flavor = "multi_thread")]
async fn the_runtime_of_a_window_starts_once() {
    let (_, core) = core();
    on_start(&core);
    on_start(&core);
    for _ in 0..400 {
        if for_host(core.host()).is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let runtime = for_host(core.host()).expect("started");
    // The test binary has no bundled plugins beside it.
    assert!(runtime.plugins().is_empty());
    tokio::time::sleep(Duration::from_millis(50)).await;
    let count = lock(registry())
        .iter()
        .filter(|entry| std::ptr::eq(entry.host.as_ptr(), Arc::as_ptr(core.host())))
        .count();
    assert_eq!(count, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn plugins_panels_opens_the_panels_window_at_the_panel_asked_for() {
    let (ui, core) = core();
    crate::panel_dock::tests::float(&core);
    core.run_command(
        rb_plugin_host::PANELS_COMMAND,
        json!({ "plugin": "hello-panel", "panel": "main" }),
    )
    .expect("runs");
    core.run_command(rb_plugin_host::PANELS_COMMAND, Value::Null)
        .expect("runs");
    let calls = ui.calls();
    assert!(calls.contains(&UiCall::PageWindow(
        PageWindow::Panels,
        "rbrowser://ui/settings.html#/panels/hello-panel.main".to_owned()
    )));
    assert!(calls.contains(&UiCall::PageWindow(
        PageWindow::Panels,
        "rbrowser://ui/settings.html#/panels".to_owned()
    )));
}

/// The `pluginPanel` messages sent to the shell pages.
fn panel_messages(ui: &RecordingUi) -> Vec<Value> {
    ui.calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Shell(script) => {
                let start = script.find("detail:")? + "detail:".len();
                let end = script.rfind("}));")?;
                serde_json::from_str::<Value>(&script[start..end]).ok()
            }
            _ => None,
        })
        .filter(|message| message["type"] == "pluginPanel")
        .map(|message| message["message"].clone())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn plugins_panels_selects_the_panel_in_a_window_already_open() {
    let (ui, core) = core();
    core.run_command(
        rb_plugin_host::PANELS_COMMAND,
        json!({ "plugin": "hello-panel", "panel": "main" }),
    )
    .expect("runs");
    core.run_command(rb_plugin_host::PANELS_COMMAND, Value::Null)
        .expect("runs");
    core.run_command(
        rb_plugin_host::PANELS_COMMAND,
        json!({ "plugin": "hello-panel", "panel": "a#b" }),
    )
    .expect("runs");
    assert_eq!(
        panel_messages(&ui),
        vec![json!({ "select": { "plugin": "hello-panel", "panel": "main" } })]
    );
}

/// RB-080-97: the panels page asked for the list while the plugins loaded, got
/// `plugins.error.not_available` and never asked again - the dock showed Findings alone. The
/// start tells every shell page once the panels are there, and then that the plugins are in
/// (`pluginsChanged`, RB-094-26: the settings pages read the plugins' keys again); a settings
/// change that changes no panel says nothing.
#[tokio::test(flavor = "multi_thread")]
async fn the_start_tells_the_panels_pages_that_the_list_is_there() {
    let (ui, core) = core();
    let changed = panel_list::changed_message();
    let plugins = live::changed_message();
    on_start(&core);
    for _ in 0..400 {
        if panel_messages(&ui).contains(&plugins) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(panel_messages(&ui), vec![changed.clone(), plugins.clone()]);
    on_user_settings_changed(&core);
    on_config_changed(&core);
    assert_eq!(panel_messages(&ui), vec![changed.clone(), plugins.clone()]);
    // A panel hidden (`plugins.panelsHidden`) is news.
    let mut user = core.user_settings();
    user.plugins.panels_hidden = vec![rb_config::PluginId("console".to_owned())];
    core.replace_user_settings(user);
    assert_eq!(panel_messages(&ui), vec![changed.clone(), plugins, changed]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_command_registered_late_joins_the_list_with_a_free_plugin_chord() {
    let (_, core) = core();
    core.register_late(|registry| {
        let def = CommandDef {
            id: CommandId::new("plugin.hello_panel.say_hello").expect("an id"),
            title_key: "plugin.hello-panel.commands.say_hello".to_owned(),
            group: Group::Plugins,
            default_bindings: Vec::new(),
            mcp: McpCoverage::Covered("hello_greet".to_owned()),
        };
        registry
            .register_plugin(def, Arc::new(|_| Ok(json!("hello"))))
            .err()
            .into_iter()
            .collect()
    });
    let rb_ipc::CoreToShell::Commands { commands, .. } = core.commands() else {
        panic!("commands");
    };
    let hello = commands
        .iter()
        .find(|info| info.id == "plugin.hello_panel.say_hello")
        .expect("registered");
    assert!(
        hello
            .default_bindings
            .iter()
            .any(|binding| binding.starts_with("Ctrl+Shift+Space X ")),
        "{:?}",
        hello.default_bindings
    );
    assert_eq!(
        core.run_command("plugin.hello_panel.say_hello", Value::Null)
            .expect("runs"),
        json!("hello")
    );
}

/// RB-080-82: the settings window and the palette showed `plugin.<id>.commands.<cmd>` - the
/// shell has the core's catalogue only. The command list carries the plugin titles.
#[tokio::test(flavor = "multi_thread")]
async fn the_command_list_carries_the_titles_of_the_plugin_commands() {
    let locale = br#"{"plugin":{"texts-list":{"commands":{"toggle":"Texts list: Toggle"}}}}"#;
    rb_i18n::register_plugin_locale("texts-list", "en", locale).expect("registers");
    let (_, core) = core();
    core.register_late(|registry| {
        let def = CommandDef {
            id: CommandId::new("plugin.texts_list.toggle").expect("an id"),
            title_key: "plugin.texts-list.commands.toggle".to_owned(),
            group: Group::Overlays,
            default_bindings: Vec::new(),
            mcp: McpCoverage::Omitted("test".to_owned()),
        };
        registry
            .register_plugin(def, Arc::new(|_| Ok(Value::Null)))
            .err()
            .into_iter()
            .collect()
    });
    let rb_ipc::CoreToShell::Commands { texts, .. } = core.commands() else {
        panic!("commands");
    };
    assert_eq!(
        texts
            .get("plugin.texts-list.commands.toggle")
            .map(String::as_str),
        Some("Texts list: Toggle")
    );
    assert!(
        texts.keys().all(|key| key.starts_with("plugin.")),
        "{texts:?}"
    );
}

/// RB-090-71 item 2: `plugins.panels` is a person's (the Panels menus, the palette, its
/// shortcut; no MCP tool runs it) - a dock folded to its rail, the start default, opens at the
/// panel asked for. A plugin command's `$open` and a check's findings keep the fold.
#[tokio::test(flavor = "multi_thread")]
async fn plugins_panels_opens_a_folded_dock_at_the_panel() {
    let mut user = rb_config::UserSettings::default();
    user.panels.dock = rb_config::PanelDock::Right;
    user.panels.dock_state = rb_config::PanelDockState::Rail;
    let setup = crate::panel_dock::tests::setup_with(user);
    setup
        .core
        .run_command(
            rb_plugin_host::PANELS_COMMAND,
            json!({ "plugin": "hello-panel", "panel": "main" }),
        )
        .expect("runs");
    let dock = crate::panel_dock::tests::docks(&setup.ui)
        .last()
        .cloned()
        .flatten()
        .expect("docked");
    assert_eq!(
        (dock.url.as_str(), dock.collapsed),
        (
            "rbrowser://ui/settings.html#/panels/hello-panel.main",
            false
        )
    );
    assert_eq!(
        setup.core.user_settings().panels.dock_state,
        rb_config::PanelDockState::Open
    );
    assert_eq!(
        panel_messages(&setup.ui),
        vec![json!({ "select": { "plugin": "hello-panel", "panel": "main" } })]
    );
    // A plugin command's `$open` is a side effect of the command: the fold stays.
    let mut user = rb_config::UserSettings::default();
    user.panels.dock = rb_config::PanelDock::Right;
    user.panels.dock_state = rb_config::PanelDockState::Rail;
    let folded = crate::panel_dock::tests::setup_with(user);
    crate::plugins::commands::open_panels(
        &folded.core,
        &json!({ "plugin": "hello-panel", "panel": "main" }),
    );
    let dock = crate::panel_dock::tests::docks(&folded.ui)
        .last()
        .cloned()
        .flatten()
        .expect("docked");
    assert!(dock.collapsed);
}
