//! The section of a Markdown heading (RB-110-15): the lines below it up to the next heading at
//! its indent, a line less indented than it or the top-level `Archive:`. A heading nests by its
//! level, not by indent, so its lines are no block of it: deleting or moving a heading takes only
//! its own line, and only a new task goes into its section. A new heading (RB-110-16) goes after
//! the heading's whole section instead - its sub-headings included ([`add_heading`]).

use crate::edit::{Refusal, archive_line, block_end, child_indent, insert, top_level_end};
use crate::format::{Document, Kind, classify, indent_width};

/// Where a new task below heading `index` goes and its indent: after the last line of the
/// heading's own section that is not blank, in the indent of the section's first line, else the
/// heading's.
#[must_use]
pub fn insertion(document: &Document, index: usize) -> (usize, String) {
    let own = document
        .parsed(index)
        .map(|parsed| parsed.indent.to_owned())
        .unwrap_or_default();
    let width = indent_width(&own);
    let archive = archive_line(document);
    let mut end = index + 1;
    let mut indent: Option<String> = None;
    for next in index + 1..document.lines.len() {
        let Some(parsed) = document.parsed(next) else {
            break;
        };
        if parsed.kind == Kind::Blank {
            continue;
        }
        let found = indent_width(parsed.indent);
        let heading = matches!(parsed.kind, Kind::Heading(_)) && found == width;
        if found < width || heading || archive == Some(next) {
            break;
        }
        indent.get_or_insert_with(|| parsed.indent.to_owned());
        end = next + 1;
    }
    (end, indent.unwrap_or(own))
}

/// The end (exclusive, blank lines after it left out) of heading `index`'s whole section: up to
/// the next heading at its indent of its level or a higher one, a less indented line or the
/// top-level `Archive:`.
fn section_end(document: &Document, index: usize) -> usize {
    let Some(own) = document.parsed(index) else {
        return index + 1;
    };
    let (width, kind) = (indent_width(own.indent), own.kind);
    let archive = archive_line(document);
    let mut end = index + 1;
    for next in index + 1..document.lines.len() {
        let Some(parsed) = document.parsed(next) else {
            break;
        };
        if parsed.kind == Kind::Blank {
            continue;
        }
        let found = indent_width(parsed.indent);
        let peer = match (parsed.kind, kind) {
            (Kind::Heading(other), Kind::Heading(mine)) => found == width && other <= mine,
            _ => false,
        };
        if found < width || peer || archive == Some(next) {
            break;
        }
        end = next + 1;
    }
    end
}

/// A new Markdown heading `line` (a whole heading line, `## Sprint`, as the person wrote it):
/// after heading `under`'s whole section ([`section_end`]) in its indent, at the end of project
/// or task `under`'s block in its children's indent, or at the top level before `Archive:`. A
/// heading without indent gets a blank line above it, as Markdown writes one. Only lines are
/// added, so the rest of the file stays byte for byte. Its line.
///
/// # Errors
///
/// `errors.no_line`, `errors.not_a_parent` (`under` a note or a blank line),
/// `errors.heading_depth` (the indent there is four columns or more: no heading any more).
pub fn add_heading(
    document: &mut Document,
    line: &str,
    under: Option<usize>,
) -> Result<usize, Refusal> {
    let (at, indent) = match under {
        Some(parent) => match document.parsed(parent).ok_or("errors.no_line")?.kind {
            Kind::Project | Kind::Task(_) => {
                (block_end(document, parent), child_indent(document, parent))
            }
            Kind::Heading(_) => {
                let indent = document
                    .parsed(parent)
                    .map(|parsed| parsed.indent.to_owned())
                    .unwrap_or_default();
                (section_end(document, parent), indent)
            }
            Kind::Note | Kind::Blank => return Err("errors.not_a_parent"),
        },
        None => (top_level_end(document), String::new()),
    };
    let text = format!("{indent}{line}");
    if !matches!(classify(&text).kind, Kind::Heading(_)) {
        return Err("errors.heading_depth");
    }
    let mut texts = Vec::new();
    if indent.is_empty() && at > 0 {
        texts.push(String::new());
    }
    texts.push(text);
    let added = at + texts.len() - 1;
    insert(document, at, texts);
    Ok(added)
}

#[cfg(test)]
#[path = "section_tests.rs"]
mod tests;
