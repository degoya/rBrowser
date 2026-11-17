//! Todo (RB-110-13, `docs/plugins.md`): the project's `.todo` files in the Todo+ format as
//! checklists, kept in the panel and over MCP without leaving the browser.
//!
//! - **The format** ([`format`]): Todo+' projects, tasks, tags and notes, and Markdown headings
//!   (RB-110-15, [`section`]), as lines, so a file read and written without a change stays byte
//!   for byte the same, and the changes ([`edit`]) touch only their own lines - a check writes
//!   `@done(...)` as Todo+ does ([`clock`]).
//! - **The files** ([`files`]): found below the project root by the setting `files`
//!   (`**/*.todo`), read with `project-read`, written with `project-write` - only in a trusted
//!   workspace, and only over the bytes read, so a file saved elsewhere meanwhile is a conflict
//!   (`plugins.error.write_conflict`, or `errors.changed` for a stale `rev`), never overwritten.
//! - **Tools** ([`tools`]): `list` (MCP `todo_list`, in pages that fit a panel message: [`page`]),
//!   `add`, `toggle`, `edit` and `archive`.
//! - **Commands**: `panel` (`Ctrl+Shift+Space X T`), `new` (`Ctrl+Shift+Space X N`, the panel at a
//!   new todo) and `archive` (`Ctrl+Shift+Space X V`).
//!
//! The logic is plain JSON on the native traits and tested natively; `guest.rs` translates it
//! into the contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

pub mod clock;
pub mod config;
pub mod edit;
pub mod files;
pub mod fold;
pub mod format;
pub mod glob;
pub mod ignore;
pub mod items;
pub mod page;
pub mod search;
pub mod section;
pub mod tools;

#[cfg(test)]
mod fake_host;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Value, json};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "todo";
/// The panel the commands show.
pub const PANEL_ID: &str = "todo";
/// The answer key asking the host to show a panel of the plugin.
pub const OPEN_KEY: &str = "$open";
/// The answer key of the posts.
pub const POST_KEY: &str = "$post";

/// The tools.
pub const LIST: &str = "list";
pub const ADD: &str = "add";
pub const TOGGLE: &str = "toggle";
pub const EDIT: &str = "edit";
pub const ARCHIVE: &str = "archive";
/// The commands.
pub const PANEL: &str = "panel";
pub const NEW: &str = "new";

/// The i18n key `plugin.todo.<name>`.
#[must_use]
pub fn key(name: &str) -> String {
    format!("plugin.{PLUGIN_ID}.{name}")
}

pub(crate) fn coded(kind: FailureKind, name: &str) -> Failure {
    Failure::coded(kind, &key(name))
}

pub(crate) fn bad_args(reason: &str) -> Failure {
    let reason: String = reason.chars().take(200).collect();
    coded(FailureKind::BadData, "errors.bad_args").with_param("reason", reason)
}

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", name.chars().take(64).collect::<String>())
}

/// `args` as a JSON object (`{}` for an empty text).
///
/// # Errors
///
/// No JSON object.
pub fn parse(args: &str) -> Result<Value, Failure> {
    if args.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(args) {
        Ok(value) if value.is_object() => Ok(value),
        _ => Err(bad_args("arguments")),
    }
}

/// The answer that opens the panel, with `message` posted to it.
fn open_panel(message: Option<Value>) -> Value {
    let mut answer = json!({ OPEN_KEY: { "panel": PANEL_ID } });
    if let Some(message) = message {
        answer[POST_KEY] = json!([{ "target": "panel", "message": message }]);
    }
    answer
}

/// The plugin.
pub struct Todo;

fn answer(value: Result<Value, Failure>) -> Result<String, Failure> {
    value.map(|value| value.to_string())
}

impl Feature for Todo {
    fn run_command(
        &mut self,
        host: &mut dyn PluginHost,
        command: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        match command {
            PANEL => answer(Ok(open_panel(None))),
            NEW => answer(Ok(open_panel(Some(json!({ "new": true }))))),
            ARCHIVE => answer(tools::archive(host, &args)),
            other => Err(unknown(other)),
        }
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        let args = parse(args)?;
        match tool {
            LIST => answer(tools::list(host, &args)),
            ADD => answer(tools::add(host, &args)),
            TOGGLE => answer(tools::toggle(host, &args)),
            EDIT => answer(tools::edit(host, &args)),
            ARCHIVE => answer(tools::archive(host, &args)),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
