//! A todo file as the panel and MCP see it: every line that is not blank, with its line number,
//! nesting depth, kind, state, a heading's level, text without tags, tags, the projects it lies
//! in and whether it is archived; the file's revision names the version shown. The answers carry them in pages
//! ([`crate::page`]).

use serde_json::{Value, json};

use crate::edit::ARCHIVE;
use crate::format::{
    Document, Kind, State, heading_text, indent_width, project_name, strip_tags, tags,
};

/// One open item of the walk: its indent width, its name when it is a project, whether it is
/// the top-level `Archive:`.
struct Open {
    width: usize,
    project: Option<String>,
    archive: bool,
}

/// The items of `document`.
#[must_use]
pub fn items(document: &Document) -> Vec<Value> {
    let mut stack: Vec<Open> = Vec::new();
    let mut items = Vec::new();
    for (index, line) in document.lines.iter().enumerate() {
        let parsed = crate::format::classify(&line.text);
        if parsed.kind == Kind::Blank {
            continue;
        }
        let width = indent_width(parsed.indent);
        while stack.last().is_some_and(|open| open.width >= width) {
            stack.pop();
        }
        let projects: Vec<&str> = stack
            .iter()
            .filter_map(|open| open.project.as_deref())
            .collect();
        let archived = stack.iter().any(|open| open.archive);
        let found: Vec<Value> = tags(parsed.rest)
            .into_iter()
            .map(|tag| json!({ "name": tag.name, "value": tag.value }))
            .collect();
        let mut item = json!({
            "line": index + 1,
            "depth": stack.len(),
            "raw": parsed.rest,
            "tags": found,
            "project": projects.join("."),
            "archived": archived,
        });
        let name = match parsed.kind {
            Kind::Project => project_name(parsed.rest).map(str::to_owned),
            _ => None,
        };
        match parsed.kind {
            Kind::Task(state) => {
                item["kind"] = json!("task");
                item["state"] = json!(state.as_str());
                item["symbol"] = json!(parsed.symbol);
                item["text"] = json!(strip_tags(parsed.rest));
            }
            Kind::Project => {
                item["kind"] = json!("project");
                item["text"] = json!(name.clone().unwrap_or_default());
            }
            Kind::Heading(level) => {
                // Edited as the whole line, its `#`s included (RB-110-15).
                item["kind"] = json!("heading");
                item["level"] = json!(level);
                item["text"] = json!(strip_tags(heading_text(parsed.rest)));
                item["raw"] = json!(&line.text[parsed.indent.len()..]);
            }
            Kind::Note | Kind::Blank => {
                item["kind"] = json!("note");
                item["text"] = json!(parsed.rest);
            }
        }
        let archive = width == 0 && name.as_deref() == Some(ARCHIVE);
        stack.push(Open {
            width,
            project: name,
            archive,
        });
        items.push(item);
    }
    items
}

/// How many tasks outside `Archive:` are open, done and cancelled, of the items of a file.
#[must_use]
pub fn counts(items: &[Value]) -> Value {
    let count = |state: State| {
        items
            .iter()
            .filter(|item| item["archived"] == false && item["state"] == state.as_str())
            .count()
    };
    json!({
        "open": count(State::Open),
        "done": count(State::Done),
        "cancelled": count(State::Cancelled),
    })
}

#[cfg(test)]
#[path = "items_tests.rs"]
mod tests;
