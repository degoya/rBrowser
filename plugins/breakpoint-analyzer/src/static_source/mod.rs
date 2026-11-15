//! The static source (RB-030-07): breakpoints the project's own files declare, beside the
//! run-time source's media queries.
//!
//! The host reads the project files the manifest's `project-read` globs and
//! `analyzer.staticGlobs` both allow (never `node_modules`, `.git` or another dot folder) and
//! hands their text over; nothing here touches a disk or runs a line of the project's code. The
//! files are read by kind:
//!
//! - `tailwind.config.{js,cjs,mjs,ts}`: Tailwind v3's `theme.screens` and `theme.extend.screens`,
//!   read as literals ([`tailwind3`], [`js`]);
//! - `.css`: Tailwind v4's `@theme { --breakpoint-*: ... }` over the defaults of every entry file
//!   that imports `tailwindcss` ([`tailwind4`]);
//! - `.scss`: Bootstrap's `$grid-breakpoints` map, or its defaults when Bootstrap is only imported
//!   ([`bootstrap`]), and `$breakpoint-*`, `$bp-*`, `$screen-*` variables;
//! - `.less`: `@breakpoint-*`, `@bp-*`, `@screen-*` variables ([`scss`]).
//!
//! The variables are a weaker, heuristic source. What cannot be read statically - a function
//! call, a variable, a range or a raw query - becomes a [`Hint`], never an error.

pub mod bootstrap;
pub mod js;
pub mod scss;
pub mod tailwind3;
pub mod tailwind4;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::parser::length;
use crate::suggest::Framework;
use crate::{PLUGIN_ID, Source};

/// The largest declared width taken seriously, as in the media query parser.
const MAX_WIDTH: f64 = 10_000.0;

/// One project file the host read.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ProjectFile {
    /// Relative to the project root, `/`-separated.
    pub path: String,
    /// Its text.
    pub text: String,
}

/// One breakpoint a project file declares: the layout changes from `width` on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declared {
    /// The first width in CSS px of the `min-width` it stands for.
    pub width: u32,
    /// Its name in the source (`md`, `3xl`).
    pub label: String,
    /// The file it comes from.
    pub path: String,
    /// The framework whose config declares it; `None` for a variable read by its name.
    pub framework: Option<Framework>,
    /// Read from a variable name only (`$bp-md`): the weaker source.
    pub heuristic: bool,
}

/// Something the static source could not read, for the wizard and the log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Hint {
    /// The file.
    pub path: String,
    /// The i18n key of the message (`plugin.breakpoint-analyzer.hints.*`).
    pub key: String,
    /// Its parameters, `path` among them.
    pub params: BTreeMap<String, String>,
}

impl Hint {
    /// The hint `kind` (`dynamic`, `range`, `presets`) about `name` in the file at `path`.
    #[must_use]
    pub fn new(path: &str, kind: &str, name: &str) -> Self {
        let mut params = BTreeMap::new();
        params.insert("path".to_owned(), path.to_owned());
        params.insert("name".to_owned(), name.to_owned());
        Self {
            path: path.to_owned(),
            key: format!("plugin.{PLUGIN_ID}.hints.{kind}"),
            params,
        }
    }
}

/// What one reader found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileScan {
    pub declared: Vec<Declared>,
    pub hints: Vec<Hint>,
}

impl FileScan {
    /// Adds the breakpoint `label` at `px` CSS px, when it is a sensible width.
    pub fn declare(
        &mut self,
        path: &str,
        label: &str,
        px: f64,
        framework: Option<Framework>,
        heuristic: bool,
    ) {
        if let Some(width) = width_of(px) {
            self.declared.push(Declared {
                width,
                label: label.to_owned(),
                path: path.to_owned(),
                framework,
                heuristic,
            });
        }
    }

    /// Adds every breakpoint and hint of `other`.
    pub fn extend(&mut self, other: FileScan) {
        self.declared.extend(other.declared);
        self.hints.extend(other.hints);
    }
}

/// The static source over every file: what they declare, the hints, and the files that declare
/// something with their count (in the order of `files`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StaticScan {
    pub declared: Vec<Declared>,
    pub hints: Vec<Hint>,
    pub sources: Vec<Source>,
}

/// What a file is read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    TailwindConfig,
    Css,
    Scss,
    Less,
}

fn kind(path: &str) -> Option<Kind> {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    let (stem, extension) = name.rsplit_once('.')?;
    match extension {
        "js" | "cjs" | "mjs" | "ts" if stem == "tailwind.config" => Some(Kind::TailwindConfig),
        "css" => Some(Kind::Css),
        "scss" => Some(Kind::Scss),
        "less" => Some(Kind::Less),
        _ => None,
    }
}

/// Reads every file; `root_font_size` converts `em` and `rem`.
#[must_use]
pub fn scan(files: &[ProjectFile], root_font_size: f64) -> StaticScan {
    let mut found = FileScan::default();
    let mut css = Vec::new();
    let mut scss = Vec::new();
    for file in files {
        match kind(&file.path) {
            Some(Kind::TailwindConfig) => {
                found.extend(tailwind3::scan(&file.path, &file.text, root_font_size));
            }
            Some(Kind::Css) => css.push(file),
            Some(Kind::Scss) => {
                scss.push(file);
                found.extend(scss::scan(&file.path, &file.text, '$', root_font_size));
            }
            Some(Kind::Less) => {
                found.extend(scss::scan(&file.path, &file.text, '@', root_font_size));
            }
            None => {}
        }
    }
    found.extend(tailwind4::scan_project(&css, root_font_size));
    found.extend(bootstrap::scan_project(&scss, root_font_size));
    let mut sources: Vec<Source> = Vec::new();
    for file in files {
        let count = found
            .declared
            .iter()
            .filter(|declared| declared.path == file.path)
            .count();
        if count > 0 && !sources.iter().any(|source| source.url == file.path) {
            sources.push(Source {
                url: file.path.clone(),
                count: u32::try_from(count).unwrap_or(u32::MAX),
            });
        }
    }
    StaticScan {
        declared: found.declared,
        hints: found.hints,
        sources,
    }
}

/// The boundary of a `min-width` of `px` CSS px: `ceil(px)`, within 1 to 10 000.
#[must_use]
pub fn width_of(px: f64) -> Option<u32> {
    let width = px.ceil();
    // Bounded to 1..=10 000, so the cast is exact.
    (1.0..=MAX_WIDTH).contains(&width).then_some(width as u32)
}

/// A length literal (`768px`, `48rem`, `"48em"`) in CSS px; quotes around it are allowed.
#[must_use]
pub fn literal_px(text: &str, root_font_size: f64) -> Option<f64> {
    let text = text.trim();
    let text = text
        .strip_prefix(['"', '\''])
        .and_then(|rest| rest.strip_suffix(['"', '\'']))
        .unwrap_or(text);
    length(text, root_font_size)
}

/// `text` without its comments: `/* */` always, `//` to the end of the line when
/// `line_comments` (SCSS, Less); strings are kept as they are.
#[must_use]
pub fn strip_comments(text: &str, line_comments: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        if let Some(open) = quote {
            out.push(c);
            if c == '\\' {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            } else if c == open || c == '\n' {
                quote = None;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"' | '\'', _) => {
                quote = Some(c);
                out.push(c);
            }
            ('/', Some('*')) => {
                chars.next();
                let mut last = ' ';
                for inner in chars.by_ref() {
                    if last == '*' && inner == '/' {
                        break;
                    }
                    last = inner;
                }
                out.push(' ');
            }
            ('/', Some('/')) if line_comments => {
                for inner in chars.by_ref() {
                    if inner == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
