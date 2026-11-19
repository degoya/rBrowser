use rb_config::SyncConfig;
use rb_sync::ElementSelector;

use super::*;
use crate::cdp::fake::FakeBrowser;

#[test]
fn the_config_follows_the_sync_settings() {
    let sync = SyncConfig {
        hover: false,
        keyboard: false,
        hover_throttle_ms: 250,
        scroll_debounce_ms: 400,
        ..SyncConfig::default()
    };
    let config = agent_config(&sync, true);
    assert!(config.active);
    assert!(config.capture.click && config.capture.scroll && config.capture.input);
    assert!(!config.capture.hover && !config.capture.keyboard);
    assert_eq!(config.hover_throttle_ms, 250);
    assert_eq!(config.scroll_debounce_ms, 400);
    // RB-090-33: anchor mode by default - the agent listens for no wheel.
    assert_eq!(config.scroll_mode, rb_sync::AgentScrollMode::Anchor);
    let wheel = SyncConfig {
        scroll_mode: rb_config::ScrollMode::Wheel,
        ..SyncConfig::default()
    };
    assert_eq!(
        agent_config(&wheel, false).scroll_mode,
        rb_sync::AgentScrollMode::Wheel
    );
}

#[test]
fn the_boot_source_puts_the_config_in_front_of_the_bundle() {
    let config = agent_config(&SyncConfig::default(), false);
    let source = boot_source("(()=>{})();", &config).expect("a bundle");
    assert!(source.starts_with("\"use strict\";globalThis.__rbAgentBoot={\"active\":false,"));
    assert!(source.ends_with("\n(()=>{})();"));
}

#[test]
fn no_bundle_no_source() {
    let config = agent_config(&SyncConfig::default(), false);
    assert_eq!(boot_source("  \n", &config), None);
}

#[test]
fn a_command_is_a_call_with_json() {
    let command = AgentCommand::Resolve {
        target: rb_sync::ElementTarget {
            hosts: Vec::new(),
            element: ElementSelector::Text {
                tag: "span".into(),
                text: "\"); alert(1); (\"".into(),
                nth: 0,
                visible_nth: None,
                within: None,
                path: "p > span".into(),
            },
            offset: rb_sync::Offset { x: 0.5, y: 0.5 },
        },
    };
    let expression = command_expression(&command).expect("serializes");
    let json = expression
        .strip_prefix("globalThis.__rbAgent.command(")
        .and_then(|rest| rest.strip_suffix(')'))
        .expect("one call");
    let back: AgentCommand = serde_json::from_str(json).expect("the argument is the JSON");
    assert_eq!(back, command);
}

#[tokio::test]
async fn inject_registers_binding_and_script_in_the_world() {
    let (fake, router) = FakeBrowser::connect();
    inject(&router, "S1", "SOURCE").await.expect("injects");
    let binding = &fake.sent("Runtime.addBinding")[0];
    assert_eq!(binding["sessionId"], "S1");
    assert_eq!(binding["params"]["name"], "__rbEmit");
    assert_eq!(binding["params"]["executionContextName"], "rbrowser");
    let script = &fake.sent("Page.addScriptToEvaluateOnNewDocument")[0];
    assert_eq!(script["params"]["worldName"], "rbrowser");
    assert_eq!(script["params"]["runImmediately"], true);
    assert_eq!(script["params"]["source"], "SOURCE");
    let attach = &fake.sent("Target.setAutoAttach")[0];
    assert_eq!(attach["params"]["flatten"], true);
    assert_eq!(attach["params"]["filter"][0]["type"], "iframe");
    assert_eq!(fake.sent("Runtime.enable").len(), 1);
}
