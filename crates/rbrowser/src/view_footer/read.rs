//! Reading a page's media queries from the document a view already shows (RB-080-47): a script
//! in an isolated world of the view walks the stylesheets of the document, its constructed
//! sheets and those of its open shadow roots - not the disabled ones - and answers each media
//! list of a sheet, an `@media` or an `@import` rule once, with the lists around it. Nothing is
//! requested: no second page load, no stylesheet fetched again. A stylesheet of another origin
//! served without CORS cannot be read from the document; it is counted as skipped and the
//! footer's tooltip says so, as it does when a page has more lists than one read takes.
//!
//! A list inside one that never holds on a screen (`media="print"`, `@media print`) is no
//! boundary of the page on a screen and is left out ([`for_screen`]).
//!
//! What the script answers is page content: it is bounded and checked here before the analyser
//! plugin sees it as one made-up stylesheet of empty `@media` rules.

use std::hash::{DefaultHasher, Hash, Hasher};

use serde::Deserialize;

use crate::analyzer::source::{Collected, Sheet};

/// The isolated world the script runs in, out of the page's reach.
pub const READ_WORLD: &str = "rbrowser-footer";
/// The most media lists taken from one page.
pub const MAX_MEDIA: usize = 4096;
/// The longest media list taken.
pub const MAX_MEDIA_LEN: usize = 512;

const SCRIPT: &str = include_str!("read.js");

/// The expression that reads the page; its value is a [`PageRead`].
#[must_use]
pub fn expression() -> String {
    format!("({})()", SCRIPT.trim_end())
}

/// What the script found in a view's document.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PageRead {
    /// The document's URL, as the page itself names it.
    pub url: String,
    /// Each media list of a sheet, an `@media` or an `@import` rule, once: the lists around it
    /// (its sheet's, the enclosing rules'), the outermost first, and itself last.
    pub media: Vec<Vec<String>>,
    /// The page has more media lists than [`MAX_MEDIA`]: the rest was left out.
    pub capped: bool,
    /// The stylesheets (and imports) the document may not read: another origin, no CORS.
    pub skipped: usize,
    /// How many stylesheets the document has.
    pub sheets: usize,
    /// The root font size (`em` / `rem` in the queries).
    pub root_font_size_px: Option<f64>,
}

/// Whether the media list `list` can hold on a screen: one of its queries names no media type,
/// `all` or `screen` - or excludes another type (`not print`).
#[must_use]
pub fn for_screen(list: &str) -> bool {
    list.split(',').any(|query| {
        let query = query.trim().to_ascii_lowercase();
        let (negated, rest) = match query.strip_prefix("not ") {
            Some(rest) => (true, rest.trim_start()),
            None => (
                false,
                query.strip_prefix("only ").unwrap_or(&query).trim_start(),
            ),
        };
        let kind: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphabetic() || *c == '-')
            .collect();
        match kind.as_str() {
            // A condition without a type: `(min-width: 1px)`, `not (hover)`.
            "" => true,
            "all" | "screen" => !negated,
            _ => negated,
        }
    })
}

/// Whether `media` is taken as it is written into a rule: not empty, not over
/// [`MAX_MEDIA_LEN`], nothing in it that could end the rule.
fn taken(media: &str) -> bool {
    !media.is_empty()
        && media.len() <= MAX_MEDIA_LEN
        && !media
            .chars()
            .any(|c| matches!(c, '{' | '}' | ';' | '@' | '\\') || c.is_control())
}

impl PageRead {
    /// The media lists that are taken: at most [`MAX_MEDIA`], each one that can hold on a
    /// screen inside lists that can ([`for_screen`]), none that could end the rule it is
    /// written into.
    pub fn conditions(&self) -> impl Iterator<Item = &str> {
        self.media
            .iter()
            .filter(|chain| chain.iter().all(|media| for_screen(media)))
            .filter_map(|chain| chain.last())
            .map(|media| media.trim())
            .filter(|media| taken(media))
            .take(MAX_MEDIA)
    }

    /// The page's queries as the analyser takes stylesheets: one sheet of empty `@media` rules.
    /// `None` for a page without a media list.
    #[must_use]
    pub fn collected(&self) -> Option<Collected> {
        let text: String = self
            .conditions()
            .map(|media| format!("@media {media}{{a{{b:c}}}}\n"))
            .collect();
        if text.is_empty() {
            return None;
        }
        let mut collected = Collected::default();
        collected.root_font_size_px = self.root_font_size_px.filter(|size| size.is_finite());
        collected.push(Sheet {
            url: format!("{}#media", super::document_of(&self.url)),
            text,
            media: None,
        });
        Some(collected)
    }

    /// What tells one read of a document from another: its media lists, what was skipped or
    /// left out, the root font size - and `config`, the mark of the config they are analysed
    /// with. The same signature needs no new analysis.
    #[must_use]
    pub fn signature(&self, config: u64) -> u64 {
        let mut hasher = DefaultHasher::new();
        for media in self.conditions() {
            media.hash(&mut hasher);
        }
        self.skipped.hash(&mut hasher);
        self.capped.hash(&mut hasher);
        self.root_font_size_px.map(f64::to_bits).hash(&mut hasher);
        config.hash(&mut hasher);
        hasher.finish()
    }
}

/// The mark of what an analysis depends on beside the page: the analyser's settings and the
/// config's breakpoints. Another mark makes every kept result stale.
#[must_use]
pub fn config_mark(config: &rb_config::ProjectConfig) -> u64 {
    let mut hasher = DefaultHasher::new();
    // A breakpoint's colour is the footer's own business: no analysis depends on it.
    let breakpoints: Vec<serde_json::Value> = config
        .breakpoints
        .iter()
        .map(|breakpoint| {
            let mut value = serde_json::json!(breakpoint);
            if let Some(fields) = value.as_object_mut() {
                fields.remove("color");
            }
            value
        })
        .collect();
    let relevant = serde_json::json!({
        "analyzer": config.analyzer,
        "breakpoints": breakpoints,
    });
    relevant.to_string().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
