//! The `rbrowser` entry merged into a client's JSON file (`mcpServers`, RB-091-09): every other
//! key and server stays as it is, a missing file is created, the same entry writes nothing and
//! another `rbrowser` entry is replaced in its place. A file that is no JSON object (comments, a
//! list) is left alone: the person adds the entry by hand.

use rb_ipc::McpFileStatus;
use serde_json::{Map, Value};

use super::{Merged, SERVER_NAME};

/// The key the servers live under in every client's JSON file the setup writes.
pub const SERVERS_KEY: &str = "mcpServers";

/// `entry` merged into `existing` (`None`: no file).
#[must_use]
pub fn merge(existing: Option<&str>, entry: &Value) -> Merged {
    let Some(text) = existing.filter(|text| !text.trim().is_empty()) else {
        let mut servers = Map::new();
        servers.insert(SERVER_NAME.to_owned(), entry.clone());
        let mut root = Map::new();
        root.insert(SERVERS_KEY.to_owned(), Value::Object(servers));
        return written(McpFileStatus::Create, &Value::Object(root), 0);
    };
    let Ok(Value::Object(mut root)) = serde_json::from_str::<Value>(text) else {
        return Merged::unreadable();
    };
    let servers = root
        .entry(SERVERS_KEY.to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    let Value::Object(servers) = servers else {
        return Merged::unreadable();
    };
    let kept = servers.keys().filter(|name| *name != SERVER_NAME).count();
    let kept = u32::try_from(kept).unwrap_or(u32::MAX);
    let status = match servers.get(SERVER_NAME) {
        Some(found) if found == entry => {
            return Merged {
                status: McpFileStatus::Unchanged,
                text: None,
                kept,
            };
        }
        Some(_) => McpFileStatus::Replace,
        None => McpFileStatus::Add,
    };
    // `preserve_order`: a replaced entry keeps its place, a new one comes last.
    servers.insert(SERVER_NAME.to_owned(), entry.clone());
    written(status, &Value::Object(root), kept)
}

fn written(status: McpFileStatus, document: &Value, kept: u32) -> Merged {
    match serde_json::to_string_pretty(document) {
        Ok(mut text) => {
            text.push('\n');
            Merged {
                status,
                text: Some(text),
                kept,
            }
        }
        Err(_) => Merged::unreadable(),
    }
}

#[cfg(test)]
#[path = "merge_json_tests.rs"]
mod tests;
