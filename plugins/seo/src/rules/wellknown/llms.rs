//! `llms.txt` in the format of llmstxt.org: an H1 with the site's name as its first line, a
//! blockquote with a short summary after it, then H2 sections whose list items are links
//! (`- [name](url): notes`). Fenced code is skipped.

/// What the format check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    NoH1,
    /// The H1 is not the first line (its line).
    H1NotFirst(usize),
    /// Another H1 (its line).
    H1Multiple(usize),
    NoSummary,
    /// A list item of a section that is no link (its line).
    BadItem(usize),
    /// A section without a link (its line and title).
    EmptySection(usize, String),
}

/// The file read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub problems: Vec<Problem>,
    /// The links of the sections' lists, as written.
    pub links: Vec<String>,
}

/// The URL of a list item `[name](url)` or `[name](url): notes`.
fn link_of(item: &str) -> Option<&str> {
    let rest = item.trim().strip_prefix('[')?;
    let (name, rest) = rest.split_once("](")?;
    let (url, after) = rest.split_once(')')?;
    let after = after.trim();
    let fits = !name.trim().is_empty()
        && !url.trim().is_empty()
        && (after.is_empty() || after.starts_with(':'));
    fits.then_some(url.trim())
}

/// The H2 section open while reading: its line, title and link count.
struct Section {
    line: usize,
    title: String,
    links: usize,
}

fn close(section: Option<Section>, parsed: &mut Parsed) {
    if let Some(section) = section
        && section.links == 0
    {
        parsed
            .problems
            .push(Problem::EmptySection(section.line, section.title));
    }
}

/// `text` held to the format.
#[must_use]
pub fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    let mut fenced = false;
    let mut first_line = true;
    let mut h1 = false;
    let mut summary_due = false;
    let mut section: Option<Section> = None;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced || trimmed.is_empty() {
            continue;
        }
        if let Some(_title) = trimmed.strip_prefix("# ") {
            if h1 {
                parsed.problems.push(Problem::H1Multiple(number));
            } else {
                h1 = true;
                summary_due = true;
                if !first_line {
                    parsed.problems.push(Problem::H1NotFirst(number));
                }
            }
            first_line = false;
            continue;
        }
        first_line = false;
        if summary_due {
            summary_due = false;
            if !trimmed.starts_with('>') {
                parsed.problems.push(Problem::NoSummary);
            }
        }
        if let Some(title) = trimmed.strip_prefix("## ") {
            close(section.take(), &mut parsed);
            section = Some(Section {
                line: number,
                title: title.trim().chars().take(80).collect(),
                links: 0,
            });
            continue;
        }
        let Some(open) = section.as_mut() else {
            continue;
        };
        let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        else {
            continue;
        };
        match link_of(item) {
            Some(url) => {
                open.links += 1;
                parsed.links.push(url.to_owned());
            }
            None => parsed.problems.push(Problem::BadItem(number)),
        }
    }
    close(section, &mut parsed);
    if !h1 {
        parsed.problems.insert(0, Problem::NoH1);
    } else if summary_due {
        parsed.problems.push(Problem::NoSummary);
    }
    parsed
}

#[cfg(test)]
#[path = "llms_tests.rs"]
mod tests;
