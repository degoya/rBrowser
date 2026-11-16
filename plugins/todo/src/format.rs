//! The Todo+ format (`fabiospampinato.vscode-todo-plus`, the owner's decision of 2026-11-16) as
//! lines: a document keeps every line's bytes and line ending, so a file read and written without
//! a change is the same file, and an edit touches only the lines it changes.
//!
//! - **Projects**: a line that is no task and ends in `:` (tags may follow), nested by indent.
//! - **Tasks**: a symbol, white space, the text. Open U+2610, done U+2714, cancelled U+2718, and
//!   the other symbols Todo+ reads ([`OPEN`], [`DONE`], [`CANCELLED`]).
//! - **Tags**: `@name` or `@name(value)` after white space ([`tags`]).
//! - **Headings** (RB-110-15): a Markdown ATX heading as CommonMark reads one - up to three
//!   columns of indent, one to six `#`, white space, a text; a closing run of `#` after white
//!   space is no part of the text ([`heading_text`]). `#tag`, `#####` alone and seven `#` are
//!   notes.
//! - **Notes**: every other line that is not blank; formatting (`*bold*`, `_italic_`,
//!   `` `code` ``) is text like any other and stays as written.

/// The byte-order mark a file may start with; kept, never part of the first line.
const BOM: char = '\u{feff}';

/// Todo+' own symbols, which new tasks and changed states of a non-bracket task get.
pub const BOX: &str = "\u{2610}";
pub const CHECK: &str = "\u{2714}";
pub const CROSS: &str = "\u{2718}";

/// The symbols of an open task Todo+ reads, the bracket forms first.
pub const OPEN: [&str; 16] = [
    "[ ]", "[]", BOX, "\u{274d}", "\u{2751}", "\u{25a0}", "\u{2b1c}", "\u{25a1}", "\u{25aa}",
    "\u{25ab}", "\u{2013}", "\u{2014}", "\u{2261}", "\u{2192}", "\u{203a}", "-",
];
/// The symbols of a done task.
pub const DONE: [&str; 7] = ["[x]", "[X]", "[+]", CHECK, "\u{2713}", "\u{2611}", "+"];
/// The symbols of a cancelled task.
pub const CANCELLED: [&str; 4] = ["[-]", CROSS, "x", "X"];

/// A task's state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Open,
    Done,
    Cancelled,
}

impl State {
    /// The name the answers use.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }

    /// The state named `name`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "open" => Some(Self::Open),
            "done" => Some(Self::Done),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// The symbol of this state: the bracket form for a bracket task, else Todo+' own.
    #[must_use]
    pub const fn symbol(self, bracket: bool) -> &'static str {
        match (self, bracket) {
            (Self::Open, true) => "[ ]",
            (Self::Done, true) => "[x]",
            (Self::Cancelled, true) => "[-]",
            (Self::Open, false) => BOX,
            (Self::Done, false) => CHECK,
            (Self::Cancelled, false) => CROSS,
        }
    }
}

/// One line: its text without the line ending, and the ending (`\n`, `\r\n`, or empty for a last
/// line without one).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Line {
    pub text: String,
    pub ending: String,
}

/// A file as lines.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Document {
    /// Whether the file starts with a byte-order mark.
    pub bom: bool,
    pub lines: Vec<Line>,
}

/// What a line is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Blank,
    Project,
    Task(State),
    /// A Markdown heading of level 1 to 6.
    Heading(u8),
    Note,
}

/// A line read: its indent, what it is, its symbol and the white space after it (a task's
/// symbol, a heading's `#`s), and the rest - a task's text with its tags, a heading's text with
/// its closing `#`s, a project's name with its colon and tags, a note's text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parsed<'a> {
    pub indent: &'a str,
    pub kind: Kind,
    pub symbol: &'a str,
    pub gap: &'a str,
    pub rest: &'a str,
}

/// One tag of a text: `@name` or `@name(value)`, with its byte range in the text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tag {
    pub name: String,
    pub value: Option<String>,
    pub start: usize,
    pub end: usize,
}

impl Document {
    /// `text` as lines; [`Document::render`] gives it back byte for byte.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let (bom, mut rest) = match text.strip_prefix(BOM) {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let mut lines = Vec::new();
        while !rest.is_empty() {
            let (line, ending, next) = match rest.find('\n') {
                Some(at) if rest[..at].ends_with('\r') => {
                    (&rest[..at - 1], "\r\n", &rest[at + 1..])
                }
                Some(at) => (&rest[..at], "\n", &rest[at + 1..]),
                None => (rest, "", ""),
            };
            lines.push(Line {
                text: line.to_owned(),
                ending: ending.to_owned(),
            });
            rest = next;
        }
        Self { bom, lines }
    }

    /// The file's text.
    #[must_use]
    pub fn render(&self) -> String {
        let mut text = String::new();
        if self.bom {
            text.push(BOM);
        }
        for line in &self.lines {
            text.push_str(&line.text);
            text.push_str(&line.ending);
        }
        text
    }

    /// The line ending most lines have; `\n` for a file without any.
    #[must_use]
    pub fn ending(&self) -> &'static str {
        let crlf = self
            .lines
            .iter()
            .filter(|line| line.ending == "\r\n")
            .count();
        let lf = self.lines.iter().filter(|line| line.ending == "\n").count();
        if crlf > lf { "\r\n" } else { "\n" }
    }

    /// Line `index` read.
    #[must_use]
    pub fn parsed(&self, index: usize) -> Option<Parsed<'_>> {
        self.lines.get(index).map(|line| classify(&line.text))
    }

    /// The width of line `index`'s indent; `None` for a blank line or none.
    #[must_use]
    pub fn width(&self, index: usize) -> Option<usize> {
        let parsed = self.parsed(index)?;
        (parsed.kind != Kind::Blank).then(|| indent_width(parsed.indent))
    }
}

/// The width of an indent: a tab counts four.
#[must_use]
pub fn indent_width(indent: &str) -> usize {
    indent.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

/// The task symbol `rest` starts with, followed by white space: `(symbol, state)`.
fn task_symbol(rest: &str) -> Option<(&'static str, State)> {
    let tables = [
        (&OPEN[..], State::Open),
        (&DONE[..], State::Done),
        (&CANCELLED[..], State::Cancelled),
    ];
    // Bracket forms before single characters, whatever their table.
    let mut found: Option<(&'static str, State)> = None;
    for (table, state) in tables {
        for symbol in table {
            let follows = rest
                .strip_prefix(symbol)
                .and_then(|after| after.chars().next())
                .is_some_and(|c| c == ' ' || c == '\t');
            if follows && found.is_none_or(|(best, _)| symbol.len() > best.len()) {
                found = Some((*symbol, state));
            }
        }
    }
    found
}

/// What `text` (a line without its ending) is.
#[must_use]
pub fn classify(text: &str) -> Parsed<'_> {
    let rest = text.trim_start_matches([' ', '\t']);
    let indent = &text[..text.len() - rest.len()];
    let blank = Parsed {
        indent,
        kind: Kind::Blank,
        symbol: "",
        gap: "",
        rest,
    };
    if rest.trim().is_empty() {
        return blank;
    }
    if let Some((symbol, state)) = task_symbol(rest) {
        let after = &rest[symbol.len()..];
        let text = after.trim_start_matches([' ', '\t']);
        return Parsed {
            kind: Kind::Task(state),
            symbol: &rest[..symbol.len()],
            gap: &after[..after.len() - text.len()],
            rest: text,
            ..blank
        };
    }
    if indent_width(indent) <= MAX_HEADING_INDENT
        && let Some(level) = heading_level(rest)
    {
        let after = &rest[level..];
        let text = after.trim_start_matches([' ', '\t']);
        return Parsed {
            kind: Kind::Heading(u8::try_from(level).expect("at most six")),
            symbol: &rest[..level],
            gap: &after[..after.len() - text.len()],
            rest: text,
            ..blank
        };
    }
    let kind = if project_name(rest).is_some() {
        Kind::Project
    } else {
        Kind::Note
    };
    Parsed { kind, ..blank }
}

/// The most columns of indent a heading has; four make a code block in CommonMark.
const MAX_HEADING_INDENT: usize = 3;

/// The level of the heading `rest` (a line without its indent) is: one to six `#`, then white
/// space and a text that is not only `#`s.
fn heading_level(rest: &str) -> Option<usize> {
    let level = rest.bytes().take_while(|&byte| byte == b'#').count();
    let after = &rest[level..];
    let spaced = after.starts_with([' ', '\t']);
    ((1..=6).contains(&level)
        && spaced
        && !heading_text(after.trim_start_matches([' ', '\t'])).is_empty())
    .then_some(level)
}

/// A heading's text without its closing `#`s and the white space around it: `Closed ##` ->
/// `Closed`, `C#` stays (the `#`s close only after white space), `##` alone is empty.
#[must_use]
pub fn heading_text(rest: &str) -> &str {
    let text = rest.trim_end_matches([' ', '\t']);
    let open = text.trim_end_matches('#');
    if open.is_empty() {
        return "";
    }
    if open.len() < text.len() && open.ends_with([' ', '\t']) {
        return open.trim_end_matches([' ', '\t']);
    }
    text
}

/// The name of a project line's rest (`Website: @critical` -> `Website`), if it is one: it ends
/// in a colon, and only tags follow.
#[must_use]
pub fn project_name(rest: &str) -> Option<&str> {
    let mut head_end = rest.len();
    for tag in tags(rest).iter().rev() {
        if rest[tag.end..head_end].trim().is_empty() {
            head_end = tag.start;
        } else {
            break;
        }
    }
    let name = rest[..head_end].trim_end().strip_suffix(':')?.trim_end();
    (!name.is_empty()).then_some(name)
}

/// The tags of `text`, in order.
#[must_use]
pub fn tags(text: &str) -> Vec<Tag> {
    let mut found = Vec::new();
    let mut previous: Option<char> = None;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        let starts = c == '@' && previous.is_none_or(char::is_whitespace);
        previous = Some(c);
        if !starts {
            continue;
        }
        let name_start = at + 1;
        let mut name_end = name_start;
        while let Some(&(index, next)) = chars.peek() {
            if next.is_whitespace() || next == '(' || next == '@' {
                break;
            }
            name_end = index + next.len_utf8();
            previous = Some(next);
            chars.next();
        }
        if name_end == name_start {
            continue;
        }
        let mut end = name_end;
        let mut value = None;
        if text[name_end..].starts_with('(')
            && let Some(close) = text[name_end..].find(')')
        {
            value = Some(text[name_end + 1..name_end + close].to_owned());
            end = name_end + close + 1;
            while chars.peek().is_some_and(|&(index, _)| index < end) {
                chars.next();
            }
            previous = Some(')');
        }
        found.push(Tag {
            name: text[name_start..name_end].to_owned(),
            value,
            start: at,
            end,
        });
    }
    found
}

/// `text` without its tags, its words joined by one space: what a list shows.
#[must_use]
pub fn strip_tags(text: &str) -> String {
    let mut plain = String::new();
    let mut from = 0;
    for tag in tags(text) {
        plain.push_str(&text[from..tag.start]);
        from = tag.end;
    }
    plain.push_str(&text[from..]);
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` without the tags `names`, each with the white space before it; the rest as written.
#[must_use]
pub fn remove_tags(text: &str, names: &[&str]) -> String {
    let mut kept = String::new();
    let mut from = 0;
    for tag in tags(text) {
        if !names.contains(&tag.name.as_str()) {
            continue;
        }
        let before = text[from..tag.start].trim_end_matches([' ', '\t']);
        kept.push_str(before);
        from = tag.end;
    }
    kept.push_str(&text[from..]);
    kept
}

/// A short, stable fingerprint of a file's bytes (FNV-1a, 64 bits, hex): what the panel names
/// the version it shows by.
#[must_use]
pub fn revision(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
