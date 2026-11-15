//! Which of the found pages a run captures, in this order: with an environment rewrite only the
//! pages of its `from` origin (nothing is captured on production by mistake), `robots.txt` (when
//! respected), the include and exclude patterns, the `hreflang` languages, `lastmod` since a
//! date, one page per template path, every n-th page, at most N pages - then the rewrite (the
//! production origin swapped for the local or staging one, the path kept).
//!
//! A pattern is a glob over the path and query (`/blog/**`, `*.html`; one that starts with
//! `http` is matched against the whole URL), or a regular expression after `re:` searched in the
//! whole URL. Globs know `*` (within a segment), `**` (across segments) and `?`.

use std::collections::BTreeSet;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::discover::Discovery;
use crate::sitemap::Entry;
use crate::url::Url;

/// The most patterns of each list.
pub const MAX_PATTERNS: usize = 64;
/// The longest pattern.
pub const MAX_PATTERN_CHARS: usize = 512;
/// The compiled size a regular expression may take.
const REGEX_SIZE_LIMIT: usize = 1 << 20;

/// What the person chose.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Selection {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    /// At most this many pages; 0 for no limit.
    pub max: usize,
    /// Every n-th page only; 0 and 1 for all.
    pub every_nth: usize,
    /// One page per template path (`/blog/*/*`).
    pub per_template: bool,
    /// `hreflang` languages (`de` takes `de-AT` too); empty for all.
    pub languages: Vec<String>,
    /// `lastmod` on or after this day (`YYYY-MM-DD`).
    pub lastmod_since: Option<String>,
    /// Leave out what `robots.txt` disallows.
    pub respect_robots: bool,
}

/// The environment rewrite: URLs of origin `from` (default: the site's) move to origin `to`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Rewrite {
    pub from: Option<String>,
    pub to: String,
}

/// One page of the run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    /// The URL to capture (after the rewrite).
    pub url: String,
    /// The URL in the sitemap, when the rewrite changed it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sitemap_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastmod: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// How many pages each step left out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub found: usize,
    /// Pages of another origin than the rewrite's `from`.
    pub origin: usize,
    pub robots: usize,
    pub pattern: usize,
    pub language: usize,
    pub lastmod: usize,
    pub template: usize,
    pub sample: usize,
    pub max: usize,
    pub selected: usize,
}

/// A pattern that cannot be used; `code` the last segment of an i18n key below
/// `plugin.sitemap-capture.errors.`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternError {
    pub code: &'static str,
    pub pattern: String,
}

enum Matcher {
    /// Against the path and query.
    Path(Regex),
    /// Against the whole URL.
    Url(Regex),
}

impl Matcher {
    fn matches(&self, url: &Url) -> bool {
        match self {
            Self::Path(regex) => regex.is_match(&url.path_and_query()),
            Self::Url(regex) => regex.is_match(&url.to_string()),
        }
    }
}

/// A glob as an anchored regular expression.
fn glob_regex(glob: &str) -> String {
    let mut out = String::from("^");
    let mut chars = glob.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                out.push_str(".*");
            }
            '*' => out.push_str("[^/]*"),
            '?' => out.push_str("[^/]"),
            other => out.push_str(&regex::escape(&other.to_string())),
        }
    }
    out.push('$');
    out
}

fn build(pattern: &str, source: &str) -> Result<Regex, PatternError> {
    regex::RegexBuilder::new(source)
        .size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map_err(|_| PatternError {
            code: "bad_pattern",
            pattern: pattern.to_owned(),
        })
}

fn compile(patterns: &[String]) -> Result<Vec<Matcher>, PatternError> {
    if patterns.len() > MAX_PATTERNS {
        return Err(PatternError {
            code: "too_many_patterns",
            pattern: String::new(),
        });
    }
    patterns
        .iter()
        .map(|pattern| {
            if pattern.is_empty() || pattern.chars().count() > MAX_PATTERN_CHARS {
                return Err(PatternError {
                    code: "bad_pattern",
                    pattern: pattern.chars().take(64).collect(),
                });
            }
            if let Some(source) = pattern.strip_prefix("re:") {
                return build(pattern, source).map(Matcher::Url);
            }
            let regex = build(pattern, &glob_regex(pattern))?;
            Ok(if pattern.starts_with("http") {
                Matcher::Url(regex)
            } else {
                Matcher::Path(regex)
            })
        })
        .collect()
}

/// The template path of `url`: segments with a digit become `*`, and so does the last one of a
/// path of two or more segments (`/blog/2024/hello` is `/blog/*/*`, `/about` stays).
#[must_use]
pub fn template(url: &Url) -> String {
    let segments: Vec<&str> = url
        .path()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let last = segments.len().saturating_sub(1);
    let parts: Vec<&str> = segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            if segment.chars().any(|c| c.is_ascii_digit()) || (index == last && last > 0) {
                "*"
            } else {
                segment
            }
        })
        .collect();
    format!("/{}", parts.join("/"))
}

/// `YYYY-MM-DD`.
fn is_day(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(at, byte)| at == 4 || at == 7 || byte.is_ascii_digit())
}

fn language_matches(wanted: &[String], language: Option<&str>) -> bool {
    let Some(language) = language else {
        return false;
    };
    wanted.iter().any(|want| {
        let want = want.trim().to_ascii_lowercase();
        language == want
            || language
                .strip_prefix(&want)
                .is_some_and(|rest| rest.starts_with(['-', '_']))
    })
}

/// The pages of `found` that `selection` keeps, rewritten by `rewrite`.
///
/// # Errors
///
/// [`PatternError`] for a pattern that does not compile, too many patterns, a `lastmod_since`
/// that is no `YYYY-MM-DD` (`bad_date`) or a rewrite `to` or `from` that is no `http(s)`
/// origin (`bad_rewrite`).
pub fn select(
    found: &Discovery,
    selection: &Selection,
    rewrite: Option<&Rewrite>,
) -> Result<(Vec<Page>, Counts), PatternError> {
    let include = compile(&selection.include)?;
    let exclude = compile(&selection.exclude)?;
    if let Some(since) = &selection.lastmod_since
        && !is_day(since)
    {
        return Err(PatternError {
            code: "bad_date",
            pattern: since.chars().take(32).collect(),
        });
    }
    let target = match rewrite {
        Some(rewrite) => Some(Url::parse(&rewrite.to).ok_or_else(|| PatternError {
            code: "bad_rewrite",
            pattern: rewrite.to.chars().take(64).collect(),
        })?),
        None => None,
    };
    let from = match rewrite.and_then(|rewrite| rewrite.from.as_deref()) {
        Some(from) => Some(Url::parse(from).ok_or_else(|| PatternError {
            code: "bad_rewrite",
            pattern: from.chars().take(64).collect(),
        })?),
        None => found.site.clone(),
    }
    .map(|url| url.origin());
    let mut counts = Counts {
        found: found.entries.len(),
        ..Counts::default()
    };
    let mut templates = BTreeSet::new();
    let mut kept: Vec<&Entry> = Vec::new();
    let every = selection.every_nth.max(1);
    let mut nth = 0usize;
    for entry in &found.entries {
        if target.is_some()
            && from
                .as_ref()
                .is_some_and(|from| *from != entry.loc.origin())
        {
            counts.origin += 1;
            continue;
        }
        if selection.respect_robots
            && found
                .robots
                .as_ref()
                .is_some_and(|robots| !robots.allows(&entry.loc))
        {
            counts.robots += 1;
            continue;
        }
        let included = include.is_empty() || include.iter().any(|m| m.matches(&entry.loc));
        if !included || exclude.iter().any(|m| m.matches(&entry.loc)) {
            counts.pattern += 1;
            continue;
        }
        if !selection.languages.is_empty()
            && !language_matches(&selection.languages, entry.language.as_deref())
        {
            counts.language += 1;
            continue;
        }
        if let Some(since) = &selection.lastmod_since {
            let day = entry.lastmod.as_deref().and_then(|text| text.get(..10));
            if !day.is_some_and(|day| is_day(day) && day >= since.as_str()) {
                counts.lastmod += 1;
                continue;
            }
        }
        if selection.per_template && !templates.insert(template(&entry.loc)) {
            counts.template += 1;
            continue;
        }
        nth += 1;
        if (nth - 1) % every != 0 {
            counts.sample += 1;
            continue;
        }
        if selection.max > 0 && kept.len() >= selection.max {
            counts.max += 1;
            continue;
        }
        kept.push(entry);
    }
    let pages: Vec<Page> = kept
        .into_iter()
        .map(|entry| {
            // Every page left is of the `from` origin (or there is none to hold it to).
            let moved = target
                .as_ref()
                .and_then(|target| entry.loc.with_origin(&target.origin()));
            Page {
                url: moved.as_ref().unwrap_or(&entry.loc).to_string(),
                sitemap_url: moved.as_ref().map(|_| entry.loc.to_string()),
                lastmod: entry.lastmod.clone(),
                language: entry.language.clone(),
            }
        })
        .collect();
    counts.selected = pages.len();
    Ok((pages, counts))
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
