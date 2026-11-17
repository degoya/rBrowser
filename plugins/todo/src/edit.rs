//! The changes the panel and MCP make to a document, each touching only its own lines: a state
//! (with `@done(...)`/`@cancelled(...)` as Todo+ writes them), a text, a new task, a deleted or
//! moved item with its children, and Todo+' "Archive": done and cancelled tasks into the
//! `Archive:` project with `@project(...)`.
//!
//! An item's block is its line and every following line indented deeper (blank lines inside
//! it included, blank lines after it not). A failure is the name of the plugin's error text
//! (`errors.<name>`).

use crate::format::{BOX, Document, Kind, Line, State, classify, indent_width, project_name};

/// The project Todo+ archives into.
pub const ARCHIVE: &str = "Archive";
/// The longest text of a task, a project or a note, in characters.
pub const MAX_TEXT_CHARS: usize = 1000;
/// The tags a change of state replaces: Todo+' time keeping.
const STATE_TAGS: [&str; 4] = ["done", "cancelled", "lasted", "wasted"];

/// The name of the error text of a refused change.
pub type Refusal = &'static str;

/// `text` as one line of a file: trimmed, not empty, no control character, not too long.
///
/// # Errors
///
/// `errors.text`.
pub fn clean_text(text: &str) -> Result<String, Refusal> {
    let text = text.trim();
    if text.is_empty()
        || text.chars().count() > MAX_TEXT_CHARS
        || text.chars().any(char::is_control)
    {
        return Err("errors.text");
    }
    Ok(text.to_owned())
}

/// The end (exclusive) of item `index`'s block.
#[must_use]
pub fn block_end(document: &Document, index: usize) -> usize {
    let Some(width) = document.width(index) else {
        return index + 1;
    };
    let mut last = index;
    for next in index + 1..document.lines.len() {
        match document.width(next) {
            None => {}
            Some(deeper) if deeper > width => last = next,
            Some(_) => break,
        }
    }
    last + 1
}

/// The state a toggle gives: open becomes done, done and cancelled become open.
#[must_use]
pub const fn toggled(state: State) -> State {
    match state {
        State::Open => State::Done,
        State::Done | State::Cancelled => State::Open,
    }
}

fn line_ending_fix(document: &mut Document, final_newline: bool) {
    let ending = document.ending();
    let last = document.lines.len().saturating_sub(1);
    for (index, line) in document.lines.iter_mut().enumerate() {
        if index < last && line.ending.is_empty() {
            line.ending = ending.to_owned();
        }
    }
    if let Some(line) = document.lines.last_mut() {
        if !final_newline {
            line.ending.clear();
        } else if line.ending.is_empty() {
            line.ending = ending.to_owned();
        }
    }
}

/// Whether the file ends in a line ending (an empty one does, as a new file will).
fn ends_in_newline(document: &Document) -> bool {
    document
        .lines
        .last()
        .is_none_or(|line| !line.ending.is_empty())
}

fn new_line(text: String) -> Line {
    Line {
        text,
        ending: String::new(),
    }
}

/// Task `index` in `state`: the symbol of that state (in the task's own style, brackets or Todo+'
/// symbols), the old time tags gone and `@done(stamp)` or `@cancelled(stamp)` added.
///
/// # Errors
///
/// `errors.no_line`, `errors.not_a_task`.
pub fn set_state(
    document: &mut Document,
    index: usize,
    state: State,
    stamp: &str,
) -> Result<(), Refusal> {
    let parsed = document.parsed(index).ok_or("errors.no_line")?;
    let Kind::Task(current) = parsed.kind else {
        return Err("errors.not_a_task");
    };
    if current == state {
        return Ok(());
    }
    let bracket = parsed.symbol.starts_with('[');
    let mut rest = crate::format::remove_tags(parsed.rest, &STATE_TAGS)
        .trim_end()
        .to_owned();
    let tag = match state {
        State::Open => None,
        State::Done => Some(format!("@done({stamp})")),
        State::Cancelled => Some(format!("@cancelled({stamp})")),
    };
    if let Some(tag) = tag {
        if !rest.is_empty() {
            rest.push(' ');
        }
        rest.push_str(&tag);
    }
    let gap = if parsed.gap.is_empty() {
        " "
    } else {
        parsed.gap
    };
    let text = format!("{}{}{gap}{rest}", parsed.indent, state.symbol(bracket));
    document.lines[index].text = text;
    Ok(())
}

/// Item `index` with the new `text`: a task's text and tags after its symbol, a project's name
/// (its colon and tags kept), a note's text, a heading's whole line after its indent (`## Text`,
/// the level free to change).
///
/// # Errors
///
/// `errors.no_line`, `errors.text`, `errors.blank_line`.
pub fn set_text(document: &mut Document, index: usize, text: &str) -> Result<(), Refusal> {
    let text = clean_text(text)?;
    let parsed = document.parsed(index).ok_or("errors.no_line")?;
    let line = match parsed.kind {
        Kind::Blank => return Err("errors.blank_line"),
        Kind::Task(_) => format!("{}{}{}{text}", parsed.indent, parsed.symbol, parsed.gap),
        Kind::Project => {
            let name = text.trim_end_matches(':').trim_end();
            if name.is_empty() {
                return Err("errors.text");
            }
            let own = project_name(parsed.rest).unwrap_or_default();
            let after = parsed.rest[own.len()..].to_owned();
            format!("{}{name}{after}", parsed.indent)
        }
        Kind::Note | Kind::Heading(_) => format!("{}{text}", parsed.indent),
    };
    // A text that turned the line into something else (a note written as `- x`) is refused.
    if std::mem::discriminant(&classify(&line).kind) != std::mem::discriminant(&parsed.kind) {
        return Err("errors.text");
    }
    document.lines[index].text = line;
    Ok(())
}

/// Deletes item `index` and its block.
///
/// # Errors
///
/// `errors.no_line`, `errors.blank_line`.
pub fn delete(document: &mut Document, index: usize) -> Result<(), Refusal> {
    if document.width(index).is_none() {
        return Err(if index < document.lines.len() {
            "errors.blank_line"
        } else {
            "errors.no_line"
        });
    }
    let final_newline = ends_in_newline(document);
    let end = block_end(document, index);
    document.lines.drain(index..end);
    line_ending_fix(document, final_newline);
    Ok(())
}

/// The first line of the sibling before (`up`) or after item `index`, if one is there.
fn sibling(document: &Document, index: usize, up: bool) -> Option<usize> {
    let width = document.width(index)?;
    if up {
        for previous in (0..index).rev() {
            match document.width(previous) {
                None => {}
                Some(found) if found == width => return Some(previous),
                Some(found) if found < width => return None,
                Some(_) => {}
            }
        }
        None
    } else {
        let start = block_end(document, index);
        (start..document.lines.len()).find_map(|next| match document.width(next) {
            None => None,
            Some(found) if found == width => Some(Some(next)),
            Some(_) => Some(None),
        })?
    }
}

/// Swaps item `index`'s block with its sibling's before (`up`) or after it; the blank lines
/// between them stay where they are. The item's new line.
///
/// # Errors
///
/// `errors.no_line`, `errors.no_sibling`.
pub fn move_item(document: &mut Document, index: usize, up: bool) -> Result<usize, Refusal> {
    if document.width(index).is_none() {
        return Err("errors.no_line");
    }
    let other = sibling(document, index, up).ok_or("errors.no_sibling")?;
    let (first, second) = if up { (other, index) } else { (index, other) };
    let first_end = block_end(document, first);
    let second_end = block_end(document, second);
    let final_newline = ends_in_newline(document);
    let mut lines: Vec<Line> = document.lines[second..second_end].to_vec();
    lines.extend_from_slice(&document.lines[first_end..second]);
    lines.extend_from_slice(&document.lines[first..first_end]);
    document.lines.splice(first..second_end, lines);
    line_ending_fix(document, final_newline);
    Ok(if up {
        first
    } else {
        first + (second_end - second) + (second - first_end)
    })
}

/// The indent one level deeper than item `index`: its first child's, else its own and the
/// file's indent step.
pub(crate) fn child_indent(document: &Document, index: usize) -> String {
    let own = document
        .parsed(index)
        .map(|parsed| parsed.indent.to_owned())
        .unwrap_or_default();
    let end = block_end(document, index);
    (index + 1..end)
        .find_map(|child| {
            let parsed = document.parsed(child)?;
            (parsed.kind != Kind::Blank).then(|| parsed.indent.to_owned())
        })
        .unwrap_or_else(|| format!("{own}{}", indent_step(document)))
}

/// The file's indent step: its smallest indent, else Todo+' two spaces.
#[must_use]
pub fn indent_step(document: &Document) -> String {
    (0..document.lines.len())
        .filter_map(|index| document.parsed(index))
        .filter(|parsed| parsed.kind != Kind::Blank && !parsed.indent.is_empty())
        .min_by_key(|parsed| indent_width(parsed.indent))
        .map_or_else(|| "  ".to_owned(), |parsed| parsed.indent.to_owned())
}

/// The top-level `Archive:` project's line, if the file has one.
#[must_use]
pub fn archive_line(document: &Document) -> Option<usize> {
    (0..document.lines.len()).find(|&index| {
        document.parsed(index).is_some_and(|parsed| {
            parsed.kind == Kind::Project
                && parsed.indent.is_empty()
                && project_name(parsed.rest) == Some(ARCHIVE)
        })
    })
}

/// Where a new top-level line goes: after the last line before `Archive:` (or the end) that is
/// not blank.
pub(crate) fn top_level_end(document: &Document) -> usize {
    let mut end = archive_line(document).unwrap_or(document.lines.len());
    while end > 0 && document.width(end - 1).is_none() {
        end -= 1;
    }
    end
}

pub(crate) fn insert(document: &mut Document, at: usize, texts: Vec<String>) {
    let final_newline = ends_in_newline(document);
    let lines: Vec<Line> = texts.into_iter().map(new_line).collect();
    document.lines.splice(at..at, lines);
    line_ending_fix(document, final_newline);
}

/// A new open task `text` (the box, U+2610) at the end of project or task `under`'s block, below
/// heading `under`'s own lines ([`crate::section::insertion`]), or at the top level before
/// `Archive:`. A `text` that is a Markdown heading (`## Sprint`) is added as that heading
/// instead ([`crate::section::add_heading`], RB-110-16). Its line.
///
/// # Errors
///
/// `errors.text`, `errors.no_line`, `errors.not_a_parent` (`under` a note or a blank line),
/// `errors.heading_depth`.
pub fn add(document: &mut Document, text: &str, under: Option<usize>) -> Result<usize, Refusal> {
    let text = clean_text(text)?;
    if matches!(classify(&text).kind, Kind::Heading(_)) {
        return crate::section::add_heading(document, &text, under);
    }
    let (at, indent) = match under {
        Some(parent) => match document.parsed(parent).ok_or("errors.no_line")?.kind {
            Kind::Project | Kind::Task(_) => {
                (block_end(document, parent), child_indent(document, parent))
            }
            Kind::Heading(_) => crate::section::insertion(document, parent),
            Kind::Note | Kind::Blank => return Err("errors.not_a_parent"),
        },
        None => (top_level_end(document), String::new()),
    };
    insert(document, at, vec![format!("{indent}{BOX} {text}")]);
    Ok(at)
}

/// The line of the project at `path` (`Website` or `Website.Design`).
#[must_use]
pub fn find_project(document: &Document, path: &str) -> Option<usize> {
    let mut stack: Vec<(usize, Option<String>)> = Vec::new();
    for index in 0..document.lines.len() {
        let Some(parsed) = document.parsed(index) else {
            continue;
        };
        if parsed.kind == Kind::Blank {
            continue;
        }
        let width = indent_width(parsed.indent);
        while stack.last().is_some_and(|(open, _)| *open >= width) {
            stack.pop();
        }
        let name = (parsed.kind == Kind::Project)
            .then(|| project_name(parsed.rest).map(str::to_owned))
            .flatten();
        stack.push((width, name));
        if parsed.kind == Kind::Project {
            let names: Vec<&str> = stack
                .iter()
                .filter_map(|(_, name)| name.as_deref())
                .collect();
            if names.join(".") == path {
                return Some(index);
            }
        }
    }
    None
}

/// A new top-level project `name` before `Archive:` (after a blank line); its line.
///
/// # Errors
///
/// `errors.text`.
pub fn add_project(document: &mut Document, name: &str) -> Result<usize, Refusal> {
    let name = clean_text(name)?;
    let name = name.trim_end_matches(':').trim_end();
    if name.is_empty() || name.contains('.') {
        return Err("errors.text");
    }
    let at = top_level_end(document);
    let mut texts = Vec::new();
    if at > 0 {
        texts.push(String::new());
    }
    texts.push(format!("{name}:"));
    let line = at + texts.len() - 1;
    insert(document, at, texts);
    Ok(line)
}

/// The names of the projects item `index` lies in, outermost first.
fn projects_of(document: &Document, index: usize) -> Vec<String> {
    let Some(mut width) = document.width(index) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for previous in (0..index).rev() {
        let Some(found) = document.width(previous) else {
            continue;
        };
        if found < width {
            width = found;
            if let Some(parsed) = document.parsed(previous)
                && parsed.kind == Kind::Project
                && let Some(name) = project_name(parsed.rest)
            {
                names.push(name.to_owned());
            }
        }
    }
    names.reverse();
    names
}

/// Todo+' "Archive": every done or cancelled task outside `Archive:` moves there with its block,
/// one level deep, tagged `@project(<its projects>)` (unless it is at the top level or has the tag
/// already). `Archive:` is made at the end when missing. How many tasks moved.
pub fn archive(document: &mut Document) -> usize {
    let archive_at = archive_line(document);
    let archive_end = archive_at.map(|line| block_end(document, line));
    let in_archive =
        |index: usize| archive_at.is_some_and(|start| index >= start && Some(index) < archive_end);
    let step = indent_step(document);
    let mut moved: Vec<(usize, usize)> = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    let mut index = 0;
    while index < document.lines.len() {
        let finished = document.parsed(index).is_some_and(|parsed| {
            matches!(parsed.kind, Kind::Task(State::Done | State::Cancelled))
        });
        if !finished || in_archive(index) {
            index += 1;
            continue;
        }
        let end = block_end(document, index);
        let indent = document
            .parsed(index)
            .map(|parsed| parsed.indent.to_owned())
            .unwrap_or_default();
        let projects = projects_of(document, index);
        for line in index..end {
            let text = &document.lines[line].text;
            let mut text = match text.strip_prefix(indent.as_str()) {
                _ if classify(text).kind == Kind::Blank => String::new(),
                Some(rest) => format!("{step}{rest}"),
                None => format!("{step}{}", text.trim_start_matches([' ', '\t'])),
            };
            let tagged = crate::format::tags(&text)
                .iter()
                .any(|tag| tag.name == "project");
            if line == index && !projects.is_empty() && !tagged {
                text = format!("{} @project({})", text.trim_end(), projects.join("."));
            }
            texts.push(text);
        }
        moved.push((index, end));
        index = end;
    }
    let count = moved.len();
    if count == 0 {
        return 0;
    }
    let final_newline = ends_in_newline(document);
    for (start, end) in moved.iter().rev() {
        document.lines.drain(*start..*end);
    }
    line_ending_fix(document, final_newline);
    let target = if let Some(line) = archive_line(document) {
        block_end(document, line)
    } else {
        let mut head = Vec::new();
        if document
            .width(document.lines.len().saturating_sub(1))
            .is_some()
        {
            head.push(String::new());
        }
        head.push(format!("{ARCHIVE}:"));
        let at = document.lines.len();
        insert(document, at, head);
        document.lines.len()
    };
    insert(document, target, texts);
    count
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
