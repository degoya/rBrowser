//! What an import says beside its requests: how many were taken, what a format has that rBrowser
//! does not (reported: the request is taken without it) and what was left out (dropped, with the
//! reason), and the secrets that became placeholders - by name, never by value.
//!
//! A note's `code` is one of [`CODES`]; the panel shows it as
//! `plugin.api-client.panel.import.notes.<code>` with `at` (the request, folder or file) and
//! `detail` (a kind, a name, a count - never a secret's value).

use serde::Serialize;

/// Every note's code.
pub const CODES: &[&str] = &[
    "auth",
    "body",
    "disabled",
    "environment",
    "examples",
    "file",
    "folder",
    "invalid",
    "item",
    "limit",
    "option",
    "script",
    "server",
    "setting",
    "skipped",
    "template",
    "tests",
    "transport",
    "variable",
];

/// Whether a note's request was taken without the thing, or the thing was left out entirely.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteKind {
    /// rBrowser has no counterpart; the rest was imported.
    Reported,
    /// Not imported, for the reason the code names.
    Dropped,
}

/// One note of the report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub kind: NoteKind,
    /// One of [`CODES`].
    pub code: String,
    /// The request, folder, environment or file it is about.
    pub at: String,
    /// What exactly: a script's kind, an auth kind, a name, a count.
    pub detail: String,
}

/// A secret of the source that became a placeholder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretNote {
    /// The request or environment it was found in.
    pub at: String,
    /// The placeholder's name: `{{$secret <name>}}`.
    pub name: String,
    /// The process variable it reads: `RB_SECRET_<NAME>`.
    pub variable: String,
}

/// The report of one import.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// The requests taken.
    pub imported: usize,
    /// The environments taken.
    pub environments: usize,
    pub notes: Vec<Note>,
    pub secrets: Vec<SecretNote>,
}

/// The most notes kept; the rest are counted in one last note.
pub const MAX_NOTES: usize = 500;

impl Report {
    /// Adds a note, at most [`MAX_NOTES`]; the same note twice is kept once.
    pub fn note(&mut self, kind: NoteKind, code: &str, at: &str, detail: &str) {
        debug_assert!(CODES.contains(&code), "unknown note code {code}");
        let note = Note {
            kind,
            code: code.to_owned(),
            at: cut(at, 200),
            detail: cut(detail, 200),
        };
        if self.notes.contains(&note) {
            return;
        }
        if self.notes.len() < MAX_NOTES {
            self.notes.push(note);
        }
    }

    /// The notes of `code`.
    #[must_use]
    pub fn of(&self, code: &str) -> Vec<&Note> {
        self.notes.iter().filter(|note| note.code == code).collect()
    }
}

/// `text` on one line, at most `max` characters.
#[must_use]
pub fn cut(text: &str, max: usize) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(max)
        .collect::<String>()
        .trim()
        .to_owned()
}
