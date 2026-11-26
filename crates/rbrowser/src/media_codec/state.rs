//! The debounce of the codec notice (RB-099-29): one notice per page and format - the page its
//! address without credentials, query and fragment - however many views show it, how often a
//! view is rebuilt or the page loads again; the findings so far go with each new one, so the
//! findings panel keeps them all (a report replaces the rule group's last part). Both are
//! bounded: the oldest page and finding go first.

use std::collections::VecDeque;

use rb_ipc::Notification;
use rb_plugin_types::Finding;
use rb_sync::MediaMessage;

use super::classify::classify;
use super::notice;

/// The most pages and formats remembered.
pub const MAX_SEEN: usize = 256;
/// The most findings kept.
pub const MAX_FINDINGS: usize = 64;

/// The page and format pairs that had their notice, oldest first.
#[derive(Debug, Default)]
pub struct Seen {
    pairs: VecDeque<(String, String)>,
}

impl Seen {
    /// Whether `format` on `page` is new; it is remembered then.
    pub fn first(&mut self, page: &str, format: &str) -> bool {
        if self
            .pairs
            .iter()
            .any(|(seen_page, seen_format)| seen_page == page && seen_format == format)
        {
            return false;
        }
        if self.pairs.len() == MAX_SEEN {
            self.pairs.pop_front();
        }
        self.pairs.push_back((page.to_owned(), format.to_owned()));
        true
    }

    /// How many pairs are remembered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether none is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

/// `url` as the page the notice is held once for.
#[must_use]
pub fn page_key(url: &str) -> String {
    let bare = rb_util::url::without_user_info(url);
    bare.split(['?', '#']).next().unwrap_or_default().to_owned()
}

/// What a new missing format brings.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The notice.
    pub notification: Notification,
    /// Every finding so far, oldest first.
    pub findings: Vec<Finding>,
}

/// The notices given and the findings so far, of one window.
#[derive(Debug, Default)]
pub struct State {
    seen: Seen,
    findings: VecDeque<Finding>,
}

impl State {
    /// What `media`, reported in `view` on `page`, brings: `None` when no format is missing or
    /// this page had the notice for its format.
    pub fn on_media(&mut self, view: u32, page: &str, media: &MediaMessage) -> Option<Outcome> {
        let missing = classify(media)?;
        if !self.seen.first(&page_key(page), &missing.format.id()) {
            return None;
        }
        if self.findings.len() == MAX_FINDINGS {
            self.findings.pop_front();
        }
        self.findings
            .push_back(notice::finding(view, page, media, &missing));
        Some(Outcome {
            notification: notice::notification(&missing, &media.src),
            findings: self.findings.iter().cloned().collect(),
        })
    }

    /// How many findings are kept.
    #[must_use]
    pub fn findings_len(&self) -> usize {
        self.findings.len()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
