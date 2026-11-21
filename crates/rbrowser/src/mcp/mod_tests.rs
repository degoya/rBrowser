use std::path::Path;
use std::sync::Arc;

use rb_commands::{CommandError, CommandRegistry, McpCoverage};
use rb_engine::ViewHost as _;
use rb_ipc::CoreToShell;
use serde_json::json;
use tokio::runtime::Handle;

use super::{
    COPY_CONFIG, SETUP, TOGGLE, config_snippet, definitions, setup_route, wsl_config_snippet,
    wsl_path,
};
use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

pub(super) fn core() -> Arc<Core> {
    core_and_ui().0
}

/// A core with the project at `root`: a config with two breakpoints written first.
pub(super) fn core_with_project(root: &Path) -> Arc<Core> {
    let config = json!({
        "version": 1,
        "breakpoints": [
            { "id": "mobile", "width": 375, "height": 812 },
            { "id": "desktop", "width": 1440, "height": 900 },
        ],
    });
    rb_config::write_config_file(root, rb_config::ConfigFile::Project, &config).expect("written");
    let loaded = crate::project::load(root).expect("loads");
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(loaded, Some(root.to_path_buf()))
        .expect("opens");
    Core::new(host, ui, Handle::current(), CoreSetup::default())
}

pub(super) fn core_and_ui() -> (Arc<Core>, Arc<RecordingUi>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui.clone(), Handle::current(), CoreSetup::default());
    (core, ui)
}

#[test]
fn the_commands_have_their_chords_and_no_tool() {
    let defs = definitions().expect("valid ids");
    let ids: Vec<&str> = defs.iter().map(|def| def.id.as_str()).collect();
    assert_eq!(ids, vec![TOGGLE, COPY_CONFIG, SETUP]);
    let chords: Vec<String> = defs
        .iter()
        .flat_map(|def| def.default_bindings.iter().map(ToString::to_string))
        .collect();
    assert_eq!(
        chords,
        vec![
            "Ctrl+Shift+Space , M",
            "Ctrl+Shift+Space , C",
            "Ctrl+Shift+Space , N"
        ]
    );
    for def in &defs {
        assert!(matches!(&def.mcp, McpCoverage::Omitted(reason) if !reason.is_empty()));
    }
}

#[test]
fn the_commands_pass_the_registry_lint() {
    let mut registry = CommandRegistry::new();
    for def in definitions().expect("valid ids") {
        let handler: rb_commands::Handler = Arc::new(|_| Ok(serde_json::Value::Null));
        registry.register(def, handler).expect("registered");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_setup_command_opens_the_client_choice_in_the_settings() {
    // RB-091-09: Settings > Automation > MCP, at `mcp.client`.
    assert_eq!(setup_route(), "settings/mcp.client");
    let (core, ui) = core_and_ui();
    core.run_command(SETUP, json!({})).expect("opens");
    assert!(ui.calls().iter().any(|call| matches!(
        call,
        UiCall::SettingsWindow(url) if url.ends_with("#/settings/mcp.client")
    )));
}

#[test]
fn the_snippet_is_stdio_without_port_or_token() {
    let line = config_snippet(
        Path::new(r"C:\Tools\rBrowser\rbrowser.exe"),
        Some(Path::new(r"C:\sites\shop")),
    );
    assert_eq!(
        line,
        r#"claude mcp add rbrowser -- "C:\Tools\rBrowser\rbrowser.exe" mcp --project "C:\sites\shop""#
    );
    assert_eq!(
        config_snippet(Path::new("rbrowser.exe"), None),
        r#"claude mcp add rbrowser -- "rbrowser.exe" mcp"#
    );
}

/// From WSL, Claude Code runs the program at its mount path and names the project by its Linux
/// path (RB-030-02).
#[test]
fn the_wsl_line_uses_the_mount_and_the_linux_path() {
    assert_eq!(
        wsl_config_snippet(
            Path::new(r"C:\Users\me\AppData\Local\rbrowser\rbrowser.exe"),
            Some(Path::new(r"\\wsl.localhost\Ubuntu\home\me\app")),
        )
        .as_deref(),
        Some(
            r#"claude mcp add rbrowser -- "/mnt/c/Users/me/AppData/Local/rbrowser/rbrowser.exe" mcp --project "/home/me/app""#
        )
    );
    assert_eq!(
        wsl_path(r"D:\sites\shop").as_deref(),
        Some("/mnt/d/sites/shop")
    );
    assert_eq!(wsl_path("C:").as_deref(), Some("/mnt/c"));
    assert_eq!(wsl_path("rbrowser.exe"), None);
    assert_eq!(wsl_path(r"\\server\share\x"), None);
    assert_eq!(wsl_config_snippet(Path::new("rbrowser.exe"), None), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_registered_command_resolves_to_a_registered_or_pending_tool() {
    let core = core();
    let CoreToShell::Commands { commands, .. } = core.commands() else {
        panic!("the commands answer");
    };
    assert!(commands.len() > 40, "{}", commands.len());
    let entries: Vec<(&str, &McpCoverage)> = commands
        .iter()
        .map(|command| (command.id.as_str(), &command.mcp))
        .collect();
    assert_eq!(rb_mcp::coverage::check(&entries), Vec::<String>::new());
    for (id, mcp) in &entries {
        if let McpCoverage::Omitted(reason) = mcp {
            assert!(!reason.contains("RB-030-01"), "{id}: {reason}");
        }
    }
    let ids: Vec<&str> = entries.iter().map(|(id, _)| *id).collect();
    assert!(
        ids.contains(&TOGGLE) && ids.contains(&COPY_CONFIG) && ids.contains(&SETUP),
        "{ids:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_navigation_commands_take_a_target() {
    let (core, ui) = core_and_ui();
    let views: Vec<_> = core.host().views().iter().map(|view| view.id).collect();
    assert!(views.len() > 1, "{views:?}");
    let first = views[0];
    core.run_command("view.back", json!({ "target": first.0 }))
        .expect("back");
    core.run_command(
        "reload.all",
        json!({ "target": "all", "ignoreCache": true }),
    )
    .expect("reload");
    let calls = ui.calls();
    let backs: Vec<_> = calls
        .iter()
        .filter_map(|call| match call {
            UiCall::Back(view) => Some(*view),
            _ => None,
        })
        .collect();
    assert_eq!(backs, vec![first]);
    let reloads = calls
        .iter()
        .filter(|call| matches!(call, UiCall::Reload(_, true)))
        .count();
    assert_eq!(reloads, views.len());
    let refused = core.run_command("view.forward", json!({ "target": "mobile" }));
    assert!(
        matches!(refused, Err(CommandError::InvalidArgs(_))),
        "{refused:?}"
    );
}
