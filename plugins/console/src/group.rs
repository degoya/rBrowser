//! Deduplication and the answer of `grouped`.
//!
//! Messages are one group when their kind, level, normalised text ([`normalize::text`]) and
//! source (`url:line`, the URL without query and fragment) are the same; a group counts its
//! messages per view, so the panel draws a dot per view that reported it. The groups keep the
//! order of their first message, oldest first, as a console does.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rb_plugin_api::ViewId;
use serde_json::{Map, Value, json};

use crate::feed::{Entry, Kind, Level};
use crate::normalize;

/// The most bytes the groups of one answer take (a panel message is at most 64 KiB).
pub const ANSWER_BUDGET: usize = 60_000;
/// The groups an answer holds when the caller names no `limit`.
pub const DEFAULT_LIMIT: usize = 200;
/// The most groups an answer holds.
pub const MAX_LIMIT: usize = 1000;
/// The longest search text.
pub const MAX_SEARCH_CHARS: usize = 200;

/// One deduplicated message.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// The stable id of its key (FNV-1a, hex).
    pub id: String,
    pub level: Level,
    pub kind: Kind,
    /// The first message's text.
    pub text: String,
    pub url: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub stack: Option<String>,
    /// Messages per view.
    pub views: BTreeMap<ViewId, usize>,
    pub first: f64,
    pub last: f64,
}

impl Group {
    /// How many messages it stands for, over every view.
    #[must_use]
    pub fn count(&self) -> usize {
        self.views.values().sum()
    }

    /// `url:line`, `url`, or nothing.
    #[must_use]
    pub fn source(&self) -> Option<String> {
        let url = self.url.as_deref()?;
        Some(match self.line {
            Some(line) => format!("{url}:{line}"),
            None => url.to_owned(),
        })
    }

    fn to_json(&self) -> Value {
        let mut group = json!({
            "id": self.id,
            "level": self.level.as_str(),
            "kind": self.kind.as_str(),
            "text": self.text,
            "count": self.count(),
            "views": self.views.keys().collect::<Vec<_>>(),
            "perView": self.views.iter().map(|(view, n)| (view.to_string(), json!(n)))
                .collect::<Map<String, Value>>(),
            "first": self.first,
            "last": self.last,
        });
        if let Some(url) = &self.url {
            group["source"] = json!({ "url": url, "line": self.line, "column": self.column });
        }
        if let Some(stack) = &self.stack {
            group["stack"] = json!(stack);
        }
        group
    }
}

/// The dedupe key of `entry`: kind, level, normalised text and source.
#[must_use]
pub fn key(entry: &Entry) -> String {
    let url = entry.url.as_deref().map_or("", normalize::source_url);
    let line = entry.line.map(|line| line.to_string()).unwrap_or_default();
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{url}:{line}",
        entry.kind.as_str(),
        entry.level.as_str(),
        normalize::text(&entry.text)
    )
}

/// `entries` (oldest first) as groups, in the order of their first message.
#[must_use]
pub fn group(entries: &[Entry]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for entry in entries {
        let key = key(entry);
        if let Some(&at) = index.get(&key) {
            let group = &mut groups[at];
            *group.views.entry(entry.view).or_default() += 1;
            group.first = group.first.min(entry.time);
            group.last = group.last.max(entry.time);
            if group.stack.is_none() {
                group.stack.clone_from(&entry.stack);
            }
            continue;
        }
        index.insert(key.clone(), groups.len());
        groups.push(Group {
            id: format!("{:016x}", normalize::fnv1a(&key)),
            level: entry.level,
            kind: entry.kind,
            text: entry.text.clone(),
            url: entry.url.clone(),
            line: entry.line,
            column: entry.column,
            stack: entry.stack.clone(),
            views: BTreeMap::from([(entry.view, 1)]),
            first: entry.time,
            last: entry.time,
        });
    }
    groups.sort_by(|a, b| a.first.total_cmp(&b.first));
    groups
}

/// Which groups an answer shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub levels: BTreeSet<Level>,
    /// Only groups at least one of these views reported; all without.
    pub views: Option<BTreeSet<ViewId>>,
    /// Only groups whose text or source holds this, ignoring case.
    pub search: Option<String>,
}

impl Filter {
    /// Whether `group` passes.
    #[must_use]
    pub fn matches(&self, group: &Group) -> bool {
        self.levels.contains(&group.level)
            && self
                .views
                .as_ref()
                .is_none_or(|views| group.views.keys().any(|view| views.contains(view)))
            && self.search.as_deref().is_none_or(|search| {
                let search = search.to_lowercase();
                group.text.to_lowercase().contains(&search)
                    || group
                        .source()
                        .is_some_and(|source| source.to_lowercase().contains(&search))
            })
    }
}

/// The page of an answer: the newest `limit` groups that pass `filter` and fit [`ANSWER_BUDGET`],
/// oldest first; how many passed; groups per level before the level filter.
#[must_use]
pub fn page(groups: &[Group], filter: &Filter, limit: usize) -> Value {
    let mut levels: BTreeMap<&str, usize> = Level::ALL.iter().map(|l| (l.as_str(), 0)).collect();
    let unlevelled = Filter {
        levels: Level::ALL.into_iter().collect(),
        ..filter.clone()
    };
    for group in groups.iter().filter(|group| unlevelled.matches(group)) {
        *levels.entry(group.level.as_str()).or_default() += 1;
    }
    let passing: Vec<&Group> = groups.iter().filter(|g| filter.matches(g)).collect();
    let mut shown = Vec::new();
    let mut bytes = 0;
    for group in passing.iter().rev().take(limit) {
        let value = group.to_json();
        bytes += value.to_string().len() + 1;
        if bytes > ANSWER_BUDGET && !shown.is_empty() {
            break;
        }
        shown.push(value);
    }
    shown.reverse();
    json!({
        "total": passing.len(),
        "shown": shown.len(),
        "truncated": shown.len() < passing.len(),
        "levels": levels,
        "entries": shown,
    })
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
