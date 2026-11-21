use serde_json::Value;

use super::*;
use crate::fake::SweepHost;
use crate::settings::{KINDS, MAX_STEP, MAX_WIDTH, MIN_WIDTH};

fn locales() -> Value {
    serde_json::from_str(include_str!("../locales/en.json")).expect("the locales are JSON")
}

/// The English text of the full key `plugin.width-sweep.<...>`.
fn text(key: &str) -> Option<String> {
    let pointer = format!("/{}", key.replace('.', "/"));
    locales().pointer(&pointer)?.as_str().map(str::to_owned)
}

#[test]
fn a_check_run_of_every_plugin_never_sweeps() {
    let mut host = SweepHost::with_views(&[1]);
    let ctx = CheckContext {
        view: Some(1),
        url: None,
        rules: KINDS.iter().map(|kind| (*kind).to_owned()).collect(),
        args: String::new(),
    };
    assert!(WidthSweep.run(&mut host, &ctx).expect("nothing").is_empty());
    assert!(host.cdp.is_empty(), "no width was set");
}

#[test]
fn the_command_and_the_tool_run_the_same_sweep_and_nothing_else_is_served() {
    let mut host = SweepHost::with_views(&[1]);
    let by_command = WidthSweep
        .run_command(&mut host, "run", r#"{"from":400,"to":420}"#)
        .expect("swept");
    let by_tool = WidthSweep
        .call_tool(&mut host, "run", r#"{"from":400,"to":420}"#)
        .expect("swept");
    assert_eq!(by_command, by_tool);
    let failure = WidthSweep
        .call_tool(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let failure = WidthSweep
        .call_tool(&mut host, "run", "[]")
        .expect_err("no object");
    assert_eq!(failure.code(), Some(codes::INVALID));
}

#[test]
fn the_manifest_declares_what_the_component_serves() {
    let manifest = include_str!("../manifest.toml");
    for line in [
        "id = \"width-sweep\"",
        "kinds = [\"check-plugin\", \"feature-plugin\"]",
        "mcp_prefix = \"sweep\"",
        "cdp = { domains = [\"Emulation\"] }",
        "max_concurrent = 2",
        "shortcut = \"Ctrl+Shift+Space B W\"",
        "shortcut = \"Ctrl+Shift+Space B Q\"",
        "context_menu = [\"run\", \"cancel\"]",
    ] {
        assert!(manifest.contains(line), "{line}");
    }
    for name in ["run", "cancel", "show"] {
        assert!(
            manifest.contains(&format!("name = \"{name}\"")),
            "tool {name}"
        );
        assert!(
            manifest.contains(&format!("tool = \"{name}\"")),
            "command {name}"
        );
    }
}

#[test]
fn every_text_the_component_names_is_in_the_locales() {
    let mut keys = vec![
        probe::PAGE_FAILED_KEY.to_owned(),
        key("name"),
        key("commands.run"),
        key("commands.cancel"),
        key("tools.run"),
        key("tools.cancel"),
        key("commands.show"),
        key("tools.show"),
        key("settings.showSeconds.label"),
        key("settings.showSeconds.description"),
        key("settings.showSeconds.help"),
    ];
    for rule in findings::describe() {
        keys.push(rule.title);
        keys.push(key(&format!("findings.{}", rule.id)));
    }
    for key in keys {
        assert!(text(&key).is_some_and(|text| !text.is_empty()), "{key}");
    }
    let overflow = text(&key("findings.overflow")).expect("text");
    assert!(overflow.starts_with("{from}-{to} px: "), "{overflow}");
}

#[test]
fn the_settings_schema_matches_the_bounds_and_is_annotated() {
    let schema: Value =
        serde_json::from_str(include_str!("../settings.schema.json")).expect("JSON");
    let properties = schema["properties"].as_object().expect("properties");
    let mut names: Vec<&str> = properties.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["from", "kinds", "showSeconds", "step", "to"]);
    for (name, property) in properties {
        assert_eq!(property["x-category"], "checks", "{name}");
        assert_eq!(property["x-section"], "width-sweep", "{name}");
        assert!(property["x-order"].is_u64(), "{name}");
    }
    for name in ["from", "to"] {
        assert_eq!(properties[name]["minimum"], MIN_WIDTH, "{name}");
        assert_eq!(properties[name]["maximum"], MAX_WIDTH, "{name}");
    }
    assert_eq!(properties["step"]["maximum"], MAX_STEP);
    assert_eq!(
        properties["kinds"]["items"]["enum"],
        serde_json::json!(KINDS)
    );
    let defaults = settings::Settings::default();
    assert_eq!(properties["from"]["default"], defaults.from);
    assert_eq!(properties["to"]["default"], defaults.to);
    assert_eq!(properties["step"]["default"], defaults.step);
    assert_eq!(properties["kinds"]["default"], serde_json::json!(KINDS));
    let shown = &properties["showSeconds"];
    assert_eq!(shown["default"], settings::SHOW_SECONDS);
    assert_eq!(shown["minimum"], settings::MIN_SHOW_SECONDS);
    assert_eq!(shown["maximum"], settings::MAX_SHOW_SECONDS);
}

#[test]
fn the_show_command_and_tool_hold_a_width() {
    let mut host = SweepHost::with_views(&[1]);
    let answer = WidthSweep
        .run_command(&mut host, "show", r#"{"target":1,"width":730}"#)
        .expect("shown");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(answer["width"], 730);
    let by_tool = WidthSweep
        .call_tool(&mut host, "show", r#"{"width":730}"#)
        .expect("shown");
    assert!(by_tool.contains("\"end\":\"elapsed\""), "{by_tool}");
}
