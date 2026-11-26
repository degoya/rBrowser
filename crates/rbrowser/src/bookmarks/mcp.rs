//! `bookmarks_list` (RB-080-48), run by the host (`rb_mcp::Backend::host_tool`): the project's
//! bookmarks as they are stored and where each leads now. A bookmark that follows the
//! environment says `relative: true` with its `path` and the `url` it has in the environment in
//! effect (absent with nothing to lie under); an absolute one says `relative: false` and its
//! `url`. A read only; `navigate` opens what it lists.

use rb_mcp::ToolFailure;
use serde_json::{Map, Value, json};

use crate::control::Core;

/// The host tool's name.
pub const LIST_TOOL: &str = "bookmarks_list";

/// Answers `bookmarks_list`.
///
/// # Errors
///
/// `args.invalid` for any argument: the tool takes none.
pub fn bookmarks_list(core: &Core, args: &Value) -> Result<Value, ToolFailure> {
    if !(args.is_null() || args.as_object().is_some_and(Map::is_empty)) {
        return Err(ToolFailure::new(
            "args.invalid",
            "bookmarks_list takes no arguments".to_owned(),
        ));
    }
    let host = core.host();
    let config = host.config();
    let environment = host.shell_state().environment;
    let base = rb_config::bookmarks::base_url(&config, environment.as_deref());
    let bookmarks: Vec<Value> = config
        .bookmarks
        .iter()
        .map(|bookmark| {
            let mut row = Map::new();
            row.insert("id".to_owned(), json!(bookmark.id));
            row.insert("label".to_owned(), json!(bookmark.label));
            if let Some(group) = &bookmark.group {
                row.insert("group".to_owned(), json!(group));
            }
            if let Some(shortcut) = &bookmark.shortcut {
                row.insert("shortcut".to_owned(), json!(shortcut));
            }
            row.insert("relative".to_owned(), json!(bookmark.is_relative()));
            if let Some(path) = bookmark.path.as_ref().filter(|_| bookmark.is_relative()) {
                row.insert("path".to_owned(), json!(path));
            }
            if let Some(url) = bookmark.url_with(base.as_deref()) {
                row.insert("url".to_owned(), json!(url));
            }
            Value::Object(row)
        })
        .collect();
    Ok(json!({
        "environment": environment,
        "base": base,
        "followEnvironment": core.user_settings().bookmarks.follow_environment,
        "bookmarks": bookmarks,
    }))
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
