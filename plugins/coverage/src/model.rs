//! What a measurement holds: per view its files with total and used units, the used segments,
//! the used CSS rules and the JavaScript functions - and how two takes of one interaction
//! session add up.

use serde::{Deserialize, Serialize};

use crate::ranges::{Segment, length, merge};

/// The most JavaScript functions kept per file.
pub const MAX_FUNCTIONS: usize = 5_000;
/// The most used rule ranges kept per file.
pub const MAX_RULES: usize = 20_000;

/// How a measurement was made.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Tracking started, every view reloaded, measured once the network is idle.
    #[default]
    Load,
    /// Tracking from `start` to `stop` while the person interacts.
    Interaction,
}

impl Mode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Interaction => "interaction",
        }
    }
}

/// CSS or JavaScript.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Css,
    Js,
}

impl Kind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Css => "css",
            Self::Js => "js",
        }
    }
}

/// Whether a view was measured.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Measured,
    /// A Firefox or WebKit view: no CDP coverage.
    Unavailable,
    /// The measurement failed (`error`).
    Failed,
}

/// One JavaScript function: its range in the script and whether it ran.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Function {
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub executed: bool,
}

/// One file of a view.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRun {
    /// The file's URL; an inline sheet or script gets `#style-<n>` / `#script-<n>` on the page's.
    pub url: String,
    pub kind: Kind,
    #[serde(default)]
    pub inline: bool,
    /// Its length in UTF-16 units (what DevTools' coverage calls bytes).
    pub total: u64,
    pub used: u64,
    /// The used segments, merged.
    #[serde(default)]
    pub segments: Vec<Segment>,
    /// CSS: the used rules as Chromium reported them (`start`, `end`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Segment>,
    /// JavaScript: the functions, the script's top level left out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<Function>,
    /// CSS: the stored text, `sources/<source>.css` in the plugin's out folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl FileRun {
    /// The unused share in percent, 0 for an empty file.
    #[must_use]
    pub fn unused_percent(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let unused = self.total.saturating_sub(self.used) as f64;
        (unused * 1000.0 / self.total as f64).round() / 10.0
    }

    /// What identifies the file across views and takes.
    #[must_use]
    pub fn same(&self, other: &Self) -> bool {
        self.url == other.url && self.kind == other.kind
    }

    /// This take added to `other`'s: the union of the used parts.
    pub fn absorb(&mut self, other: &Self) {
        self.total = self.total.max(other.total);
        let mut segments = std::mem::take(&mut self.segments);
        segments.extend_from_slice(&other.segments);
        self.segments = merge(segments);
        self.used = length(&self.segments).min(self.total);
        let mut rules = std::mem::take(&mut self.rules);
        rules.extend_from_slice(&other.rules);
        rules.sort_unstable();
        rules.dedup();
        rules.truncate(MAX_RULES);
        self.rules = rules;
        for function in &other.functions {
            let known = self
                .functions
                .iter()
                .position(|known| known.start == function.start && known.end == function.end);
            match known {
                Some(at) => self.functions[at].executed |= function.executed,
                None if self.functions.len() < MAX_FUNCTIONS => {
                    self.functions.push(function.clone());
                }
                None => {}
            }
        }
        if self.source.is_none() {
            self.source.clone_from(&other.source);
        }
    }
}

/// One view's measurement.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewRun {
    pub view: u32,
    pub label: String,
    pub breakpoint: String,
    pub width: u32,
    pub engine: String,
    /// The page's URL when measured.
    pub url: String,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub files: Vec<FileRun>,
}

impl ViewRun {
    /// The breakpoint's name, else the view's label.
    #[must_use]
    pub fn name(&self) -> &str {
        if self.breakpoint.is_empty() {
            &self.label
        } else {
            &self.breakpoint
        }
    }

    /// Adds a take's files to this view's.
    pub fn absorb(&mut self, take: &Self) {
        if !take.url.is_empty() {
            self.url.clone_from(&take.url);
        }
        if take.status != Status::Measured {
            self.status = take.status;
            self.error.clone_from(&take.error);
        }
        for file in &take.files {
            match self.files.iter_mut().find(|known| known.same(file)) {
                Some(known) => known.absorb(file),
                None => self.files.push(file.clone()),
            }
        }
    }
}

/// A measurement over the views.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub mode: Mode,
    /// Unix seconds.
    pub started: u64,
    #[serde(default)]
    pub finished: u64,
    /// An interaction session still tracking.
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub views: Vec<ViewRun>,
}

impl Run {
    /// Adds the views of a take.
    pub fn absorb(&mut self, take: &Self) {
        for view in &take.views {
            match self.views.iter_mut().find(|known| known.view == view.view) {
                Some(known) => known.absorb(view),
                None => self.views.push(view.clone()),
            }
        }
        self.finished = self.finished.max(take.finished);
    }

    /// The measured views.
    pub fn measured(&self) -> impl Iterator<Item = &ViewRun> {
        self.views
            .iter()
            .filter(|view| view.status == Status::Measured)
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
