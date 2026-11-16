use serde_json::{Value, json};

use super::*;
use crate::testing::{host, png};

fn answer(text: &str) -> Value {
    serde_json::from_str(text).expect("answer")
}

#[test]
fn open_last_opens_the_editor_at_the_newest_screenshot() {
    let shot = ".rbrowser/out/screenshots/2026-11-12_10-00-00/mobile.png";
    let mut host = host().file(shot, &png(2, 2, |_, _| [0, 0, 0, 255]));
    let reply = answer(
        &ScreenshotEditor
            .run_command(&mut host, "open_last", "{}")
            .expect("open"),
    );
    assert_eq!(reply["image"], shot);
    assert_eq!(reply["$open"], json!({ "panel": "editor" }));
    assert_eq!(
        reply["$post"],
        json!([{ "target": "panel", "message": { "open": shot } }])
    );
}

#[test]
fn open_last_without_a_screenshot_still_opens_the_editor() {
    let reply = answer(
        &ScreenshotEditor
            .run_command(&mut host(), "open_last", "")
            .expect("open"),
    );
    assert_eq!(reply["image"], Value::Null);
    assert_eq!(reply["$open"]["panel"], "editor");
}

#[test]
fn copy_asks_the_open_editor_for_the_clipboard() {
    let reply = answer(
        &ScreenshotEditor
            .run_command(&mut host(), "copy", "{}")
            .expect("copy"),
    );
    assert_eq!(
        reply["$post"],
        json!([{ "target": "panel", "message": { "copy": true } }])
    );
}

#[test]
fn unknown_commands_and_tools_are_refused() {
    let mut host = host();
    for refused in [
        ScreenshotEditor.run_command(&mut host, "delete", "{}"),
        ScreenshotEditor.call_tool(&mut host, "@page", "{}"),
        ScreenshotEditor.call_tool(&mut host, "render", "{}"),
    ] {
        assert_eq!(
            refused.expect_err("unknown").code(),
            Some("plugins.error.unknown_contribution")
        );
    }
}

/// The manifest names what this file serves: its tools, its commands and the panel.
#[test]
fn the_manifest_declares_what_the_component_serves() {
    let manifest: toml_lite::Manifest = toml_lite::read(include_str!("../manifest.toml"));
    assert_eq!(manifest.id, PLUGIN_ID);
    assert_eq!(manifest.tools, vec!["annotate", "load", "list"]);
    assert_eq!(manifest.commands, vec!["open_last", "copy"]);
    assert_eq!(manifest.panels, vec![PANEL]);
}

/// The few manifest lines the test above reads, without a TOML parser.
mod toml_lite {
    pub struct Manifest {
        pub id: String,
        pub tools: Vec<String>,
        pub commands: Vec<String>,
        pub panels: Vec<String>,
    }

    fn quoted(line: &str) -> String {
        line.split('"').nth(1).unwrap_or_default().to_owned()
    }

    pub fn read(text: &str) -> Manifest {
        let mut manifest = Manifest {
            id: String::new(),
            tools: Vec::new(),
            commands: Vec::new(),
            panels: Vec::new(),
        };
        let mut table = "";
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("[[") || (line.starts_with('[') && !line.starts_with("[[")) {
                table = line;
            } else if line.starts_with("id = ") {
                match table {
                    "[[commands]]" => manifest.commands.push(quoted(line)),
                    "[[panels]]" => manifest.panels.push(quoted(line)),
                    "" => manifest.id = quoted(line),
                    _ => {}
                }
            } else if line.starts_with("name = ") && table == "[[tools]]" {
                manifest.tools.push(quoted(line));
            }
        }
        manifest
    }
}
