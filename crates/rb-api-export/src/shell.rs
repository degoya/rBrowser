//! A command line for a shell: each word quoted so the program gets it as written, the lines
//! continued in the shell's way.
//!
//! - **POSIX**: a plain word bare, the rest in single quotes (`'` as `'\''`), an environment
//!   variable as `"${name}"` between them.
//! - **PowerShell** (7.3 or later, which passes a native program's arguments as written): a plain
//!   word bare, the rest in single quotes (its four single quote characters doubled); a word with
//!   an environment variable in double quotes, `` ` `` before `` ` ``, `$` and the double
//!   quotes, the variable as `${env:name}`.
//! - **cmd**: the program's rules (MSVCRT: `"` as `\"`, backslashes before a quote doubled),
//!   then `^` before every character `cmd` itself would read - the quotes included, as Chrome's
//!   "Copy as cURL (cmd)" does - so no state of `cmd` depends on the text; an environment
//!   variable as `%name%`; in a script (`batch`) a `%` of the text as `%%`. `cmd` holds no line
//!   break in an argument: each becomes a space and [`Renderer::altered`] says so.

use crate::Shell;
use crate::word::{Seg, Word};

/// Quotes words for one shell.
pub(crate) struct Renderer {
    pub shell: Shell,
    /// For a script file rather than a prompt (`cmd`: `%` as `%%`).
    pub batch: bool,
    /// A line break was put as a space (`cmd`).
    pub altered: bool,
}

fn bare(text: &str, extra: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._/:+-".contains(c) || extra.contains(c))
}

/// PowerShell's single quote characters: `'` and the typographic ones it reads the same.
const PS_SINGLE: [char; 5] = ['\'', '\u{2018}', '\u{2019}', '\u{201a}', '\u{201b}'];
/// What PowerShell reads inside double quotes.
const PS_DOUBLE: [char; 6] = ['`', '$', '"', '\u{201c}', '\u{201d}', '\u{201e}'];
/// What `cmd` reads on its line, outside the program's rules.
const CMD_META: [char; 10] = ['^', '&', '|', '<', '>', '(', ')', '"', '!', '%'];

impl Renderer {
    pub fn new(shell: Shell, batch: bool) -> Self {
        Self {
            shell,
            batch,
            altered: false,
        }
    }

    /// `word` quoted.
    pub fn quote(&mut self, word: &Word) -> String {
        match self.shell {
            Shell::Posix => posix(word),
            Shell::PowerShell => powershell(word),
            Shell::Cmd => self.cmd(word),
        }
    }

    /// The line continuation, with the space before it.
    pub const fn continuation(&self) -> &'static str {
        match self.shell {
            Shell::Posix => " \\",
            Shell::PowerShell => " `",
            Shell::Cmd => " ^",
        }
    }

    /// The comment of a line in a script.
    pub const fn comment(&self) -> &'static str {
        match self.shell {
            Shell::Posix | Shell::PowerShell => "#",
            Shell::Cmd => "REM",
        }
    }

    /// `groups` as one command: the first on the first line, each other on a line of its own,
    /// indented, the lines continued.
    pub fn lines(&mut self, groups: &[Vec<Word>]) -> String {
        let mut lines = Vec::with_capacity(groups.len());
        for (index, group) in groups.iter().enumerate() {
            let words: Vec<String> = group.iter().map(|word| self.quote(word)).collect();
            let indent = if index == 0 { "" } else { "  " };
            lines.push(format!("{indent}{}", words.join(" ")));
        }
        lines.join(&format!("{}\n", self.continuation()))
    }

    fn cmd(&mut self, word: &Word) -> String {
        let mut word = word.clone();
        for seg in &mut word.segs {
            if let Seg::Text(text) = seg
                && text.contains(['\r', '\n'])
            {
                *text = text.replace("\r\n", " ").replace(['\r', '\n'], " ");
                self.altered = true;
            }
        }
        if let Some(text) = word.text_only()
            && bare(&text, "")
        {
            return text;
        }
        // The program's rules first: (piece, protected) - a variable is no text to escape.
        let mut quoted: Vec<(String, bool)> = vec![("\"".to_owned(), false)];
        let mut backslashes = 0usize;
        for seg in &word.segs {
            match seg {
                Seg::Text(text) => {
                    let mut out = String::new();
                    for c in text.chars() {
                        match c {
                            '\\' => backslashes += 1,
                            '"' => {
                                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                                out.push('"');
                                backslashes = 0;
                            }
                            other => {
                                out.push_str(&"\\".repeat(backslashes));
                                out.push(other);
                                backslashes = 0;
                            }
                        }
                    }
                    quoted.push((out, false));
                }
                Seg::Env(name) => {
                    quoted.push(("\\".repeat(backslashes), false));
                    backslashes = 0;
                    quoted.push((format!("%{name}%"), true));
                }
            }
        }
        quoted.push((format!("{}\"", "\\".repeat(backslashes * 2)), false));
        let mut out = String::new();
        for (piece, protected) in quoted {
            if protected {
                out.push_str(&piece);
                continue;
            }
            for c in piece.chars() {
                if c == '%' && self.batch {
                    out.push_str("%%");
                } else if CMD_META.contains(&c) {
                    out.push('^');
                    out.push(c);
                } else {
                    out.push(c);
                }
            }
        }
        out
    }
}

fn posix(word: &Word) -> String {
    if let Some(text) = word.text_only() {
        if bare(&text, "@%=,") {
            return text;
        }
        if text.is_empty() {
            return "''".to_owned();
        }
    }
    let mut out = String::new();
    for seg in &word.segs {
        match seg {
            Seg::Text(text) if bare(text, "@%=,") => out.push_str(text),
            Seg::Text(text) => {
                out.push('\'');
                out.push_str(&text.replace('\'', "'\\''"));
                out.push('\'');
            }
            Seg::Env(name) => out.push_str(&format!("\"${{{name}}}\"")),
        }
    }
    out
}

fn powershell(word: &Word) -> String {
    if let Some(text) = word.text_only() {
        if bare(&text, "=") && !text.starts_with('@') {
            return text;
        }
        let mut out = String::from("'");
        for c in text.chars() {
            if PS_SINGLE.contains(&c) {
                out.push(c);
            }
            out.push(c);
        }
        out.push('\'');
        return out;
    }
    let mut out = String::from("\"");
    for seg in &word.segs {
        match seg {
            Seg::Text(text) => {
                for c in text.chars() {
                    if PS_DOUBLE.contains(&c) {
                        out.push('`');
                    }
                    out.push(c);
                }
            }
            Seg::Env(name) => out.push_str(&format!("${{env:{name}}}")),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
