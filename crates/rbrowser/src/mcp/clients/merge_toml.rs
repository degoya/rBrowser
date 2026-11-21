//! The `rbrowser` table merged into Codex's `config.toml` (`[mcp_servers.rbrowser]`, RB-091-09),
//! on the text: every other line - comments, settings, other servers - stays exactly as it is.
//! A missing file is created, the same table writes nothing, another `rbrowser` table (with its
//! sub-tables such as `[mcp_servers.rbrowser.env]`) is replaced in its place. A form the text
//! cannot merge safely - `mcp_servers` as an inline table or dotted keys - is left alone: the
//! person adds the entry by hand.

use std::collections::BTreeSet;

use rb_ipc::McpFileStatus;

use super::{Merged, SERVER_NAME};

/// The table every server lives under.
const SERVERS: &str = "mcp_servers";

/// The `rbrowser` table for `command`, ending with a newline.
#[must_use]
pub fn entry(command: &str) -> String {
    format!(
        "[{SERVERS}.{SERVER_NAME}]\ncommand = {}\nargs = [\"mcp\"]\n",
        basic_string(command)
    )
}

/// `text` as a TOML basic string.
fn basic_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if control.is_control() => {
                out.push_str(&format!("\\u{:04X}", u32::from(control)));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// `entry` merged into `existing` (`None`: no file).
#[must_use]
pub fn merge(existing: Option<&str>, entry: &str) -> Merged {
    let Some(text) = existing.filter(|text| !text.trim().is_empty()) else {
        return Merged {
            status: McpFileStatus::Create,
            text: Some(entry.to_owned()),
            kept: 0,
        };
    };
    let lines: Vec<&str> = text.lines().collect();
    let Some(scan) = scan(&lines) else {
        return Merged::unreadable();
    };
    let kept = u32::try_from(scan.others.len()).unwrap_or(u32::MAX);
    let Some((start, end)) = scan.ours else {
        let mut out = text.to_owned();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(entry);
        return Merged {
            status: McpFileStatus::Add,
            text: Some(out),
            kept,
        };
    };
    if meaningful(&lines[start..end]) == meaningful(&entry.lines().collect::<Vec<_>>()) {
        return Merged {
            status: McpFileStatus::Unchanged,
            text: None,
            kept,
        };
    }
    let mut out = String::new();
    for line in &lines[..start] {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(entry);
    // The blank lines before the next table are still there.
    for line in &lines[end..] {
        out.push_str(line);
        out.push('\n');
    }
    Merged {
        status: McpFileStatus::Replace,
        text: Some(out),
        kept,
    }
}

/// Where the `rbrowser` table is and which other servers there are.
struct Scan {
    /// The lines of the `rbrowser` table and its sub-tables, up to (without) the blank lines and
    /// comments before the next table.
    ours: Option<(usize, usize)>,
    /// The other servers' names.
    others: BTreeSet<String>,
}

/// `None` when the file holds a form the text cannot merge.
fn scan(lines: &[&str]) -> Option<Scan> {
    let mut ours: Option<(usize, usize)> = None;
    let mut others = BTreeSet::new();
    // The table the current line belongs to.
    let mut table: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(header) = header(trimmed) {
            table = header;
            let in_ours = table.len() >= 2 && table[0] == SERVERS && table[1] == SERVER_NAME;
            let current = ours;
            match (current, in_ours) {
                (None, true) => ours = Some((index, lines.len())),
                // The table again after another one: an unusual layout.
                (Some((_, end)), true) if end != lines.len() => return None,
                (Some((start, end)), false) if end == lines.len() && index > start => {
                    ours = Some((start, index));
                }
                _ => {}
            }
            if table.len() >= 2 && table[0] == SERVERS && table[1] != SERVER_NAME {
                others.insert(table[1].clone());
            }
            continue;
        }
        if is_filler(trimmed) {
            continue;
        }
        let key = key_path(trimmed);
        // `mcp_servers = {...}` or `mcp_servers.x.command = ...` at the top.
        if table.is_empty() && key.first().is_some_and(|first| first == SERVERS) {
            return None;
        }
        // `rbrowser = {...}` or `rbrowser.command = ...` under `[mcp_servers]`.
        if table.len() == 1 && table[0] == SERVERS {
            if key.first().is_some_and(|first| first == SERVER_NAME) {
                return None;
            }
            if let Some(first) = key.first() {
                others.insert(first.clone());
            }
        }
    }
    // Blank lines and comments before the next table belong to it.
    let ours = ours.map(|(start, mut end)| {
        while end > start + 1 && is_filler(lines[end - 1]) {
            end -= 1;
        }
        (start, end)
    });
    Some(Scan { ours, others })
}

/// The key path of a `[table]` header, its quotes taken off; `None` for any other line (an
/// array of tables `[[...]]` is a header too).
fn header(line: &str) -> Option<Vec<String>> {
    let inner = line
        .strip_prefix("[[")
        .and_then(|rest| rest.split("]]").next())
        .or_else(|| {
            line.strip_prefix('[')
                .and_then(|rest| rest.split(']').next())
        })?;
    Some(segments(inner))
}

/// The key path left of a `key = value` line.
fn key_path(line: &str) -> Vec<String> {
    line.split('=').next().map(segments).unwrap_or_default()
}

fn segments(path: &str) -> Vec<String> {
    path.split('.')
        .map(|segment| segment.trim().trim_matches(['"', '\'']).to_owned())
        .filter(|segment| !segment.is_empty())
        .collect()
}

/// A blank line or a comment.
fn is_filler(line: &str) -> bool {
    let line = line.trim();
    line.is_empty() || line.starts_with('#')
}

/// The lines that carry something, trimmed: what two tables are compared by.
fn meaningful<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !is_filler(line))
        .collect()
}

#[cfg(test)]
#[path = "merge_toml_tests.rs"]
mod tests;
