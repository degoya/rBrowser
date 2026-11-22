use std::time::Duration;

use rb_commands::CommandRegistry;
use rb_mcp::scripts::codes;

use super::*;
use crate::host::ui::recording::UiCall;

#[test]
fn the_command_sits_in_the_plugins_group_and_names_its_tool() {
    let defs = definitions().expect("a valid id");
    assert_eq!(defs.len(), 1);
    let def = &defs[0];
    assert_eq!(def.id.as_str(), RUN_SCRIPT);
    assert_eq!(def.group, Group::Plugins);
    let chords: Vec<String> = def
        .default_bindings
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(chords, ["Ctrl+Shift+Space X J"]);
    assert_eq!(def.mcp, McpCoverage::Covered(TOOL.to_owned()));
}

#[test]
fn the_command_passes_the_registry_lint() {
    let mut registry = CommandRegistry::new();
    for def in definitions().expect("a valid id") {
        let handler: Handler = Arc::new(|_| Ok(Value::Null));
        registry.register(def, handler).expect("registered");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_command_opens_the_setting_and_runs_no_script() {
    let (core, ui) = super::super::tests::core_and_ui();
    core.run_command(RUN_SCRIPT, json!({})).expect("opens");
    assert!(ui.calls().iter().any(|call| matches!(
        call,
        UiCall::SettingsWindow(url) if url.contains(ENABLED_KEY)
    )));
}

#[test]
fn the_wait_for_a_turn_and_the_first_compile_count_against_the_runs_time() {
    // One deadline from the run's arrival, as rb-mcp's own timer starts then.
    let second = Duration::from_secs(1);
    assert_eq!(remaining(30 * second, 2 * second), Some(28 * second));
    assert_eq!(remaining(30 * second, 30 * second), None);
    assert_eq!(remaining(30 * second, 31 * second), None);
}

#[test]
fn the_sandboxs_stops_keep_their_meaning() {
    assert_eq!(stop_of(RunError::Timeout), ScriptStop::Timeout);
    assert_eq!(stop_of(RunError::OutOfMemory), ScriptStop::OutOfMemory);
    let failure = format!(r#"{{"code":"{}"}}"#, codes::SYNTAX);
    assert_eq!(
        stop_of(RunError::Script(failure.clone())),
        ScriptStop::Script(failure)
    );
    assert_eq!(
        stop_of(RunError::Engine("trap".to_owned())),
        ScriptStop::Engine("trap".to_owned())
    );
}
