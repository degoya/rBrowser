//! The test plugin of the plugin runtime (RB-060-12, `docs/plugins.md`, "Page scripts and
//! panels"). It runs every path once and is never shipped:
//!
//! - the MCP tool `hello_greet` and the command `plugin.hello_panel.say_hello` ([`greet`]) answer
//!   the greeting and post it to the page scripts (`target` of the arguments, `"all"` without one)
//!   and to the panel;
//! - a page script's request (`call-tool` with `@page`, [`page_message`]) is answered with a
//!   greeting for that page and posted to the panel.
//!
//! The logic is plain JSON and tested natively; `guest.rs` translates it into the contract's
//! vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

use serde_json::{Value, json};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "hello-panel";
/// The tool a page script's request reaches `call-tool` with (`rb_plugin_host::PAGE_MESSAGE_TOOL`).
pub const PAGE_TOOL: &str = "@page";
/// The longest name a greeting takes.
pub const MAX_NAME_CHARS: usize = 64;

/// Why a call failed: an i18n key of the host (`plugins.error.*`) or the plugin, and parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub params: Vec<(String, String)>,
}

fn unknown(name: &str) -> Refusal {
    Refusal {
        code: "plugins.error.unknown_contribution",
        params: vec![("name".to_owned(), name.chars().take(64).collect())],
    }
}

fn bad_data() -> Refusal {
    Refusal {
        code: "plugins.error.invalid",
        params: vec![("reason".to_owned(), "arguments".to_owned())],
    }
}

/// `greet` (tool) and `say_hello` (command): `{"greeting", "$post": [...]}` - the badge for the
/// views of `target` (all without one) and a line for the panel.
///
/// # Errors
///
/// The arguments are no JSON object.
pub fn greet(args: &str) -> Result<String, Refusal> {
    let args: Value = serde_json::from_str(args).map_err(|_| bad_data())?;
    if !args.is_object() {
        return Err(bad_data());
    }
    let name: String = args["name"]
        .as_str()
        .unwrap_or("rBrowser")
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let target = match &args["target"] {
        Value::Number(view) => json!(view),
        _ => json!("all"),
    };
    let greeting = format!("Hello, {name}!");
    Ok(json!({
        "greeting": greeting,
        "$post": [
            { "target": target, "message": { "show": greeting } },
            { "target": "panel", "message": { "greeted": name } },
        ],
    })
    .to_string())
}

/// A page script's request `{"view", "message": {"url", "title"}}`: the greeting for the page,
/// and the page for the panel.
///
/// # Errors
///
/// The arguments are not the host's `@page` shape.
pub fn page_message(args: &str) -> Result<String, Refusal> {
    let args: Value = serde_json::from_str(args).map_err(|_| bad_data())?;
    let view = args["view"].as_u64().ok_or_else(bad_data)?;
    let message = &args["message"];
    let url: String = message["url"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(2048)
        .collect();
    let title: String = message["title"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    Ok(json!({
        "greeting": format!("Hello from the component, view {view}"),
        "$post": [{ "target": "panel", "message": { "page": url, "title": title, "view": view } }],
    })
    .to_string())
}

/// `run-command`: `say_hello`.
///
/// # Errors
///
/// An unknown command, or [`greet`]'s.
pub fn run_command(command: &str, args: &str) -> Result<String, Refusal> {
    match command {
        "say_hello" => greet(args),
        other => Err(unknown(other)),
    }
}

/// `call-tool`: `greet`, and the page scripts' requests.
///
/// # Errors
///
/// An unknown tool, or the tool's.
pub fn call_tool(tool: &str, args: &str) -> Result<String, Refusal> {
    match tool {
        "greet" => greet(args),
        PAGE_TOOL => page_message(args),
        other => Err(unknown(other)),
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
