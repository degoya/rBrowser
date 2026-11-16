//! The footer's own results (RB-080-47): what was read from the documents the views show, kept
//! per document - the [`MAX_DOCUMENTS`] used last, and always those a view shows - beside the
//! project-wide result of Analyze Breakpoints, which they never touch. A view takes the result
//! of its document; a view that comes back to a document finds it again.
//!
//! Which of the two a footer names is [`choose`]: Analyze Breakpoints fetched the stylesheets,
//! the footer's read sees only what the document may read itself, so an incomplete or empty
//! read never replaces the other.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use rb_ipc::DetectedBreakpoint;

/// How many documents are kept beside those the views show; the one used longest ago goes
/// first.
pub const MAX_DOCUMENTS: usize = 32;

/// What one document's stylesheets say.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageQueries {
    /// The boundaries of the document's media queries.
    pub detected: Vec<DetectedBreakpoint>,
    /// The stylesheets that could not be read (another origin).
    pub skipped: usize,
    /// More media lists than one read takes: the rest was left out.
    pub capped: bool,
    /// The signature of the read they come from and of the config it was analysed with.
    pub signature: u64,
}

impl PageQueries {
    /// Whether the read saw every stylesheet and every media list of the document.
    pub fn complete(&self) -> bool {
        self.skipped == 0 && !self.capped
    }
}

/// The results per document.
#[derive(Clone, Debug, Default)]
pub struct Results {
    pages: BTreeMap<String, PageQueries>,
    /// The documents, the one used longest ago first.
    order: VecDeque<String>,
}

impl Results {
    /// The result of `document`.
    pub fn get(&self, document: &str) -> Option<&PageQueries> {
        self.pages.get(document)
    }

    /// A footer names the result of `document`: it is the one used last now.
    pub fn touch(&mut self, document: &str) {
        if self.pages.contains_key(document)
            && self.order.back().map(String::as_str) != Some(document)
        {
            self.order.retain(|known| known != document);
            self.order.push_back(document.to_owned());
        }
    }

    /// Keeps `queries` for `document`; `false` when that is what was kept already. Beyond
    /// [`MAX_DOCUMENTS`] the document used longest ago goes - never one of `shown`, the
    /// documents the views show.
    pub fn put(&mut self, document: &str, queries: PageQueries, shown: &BTreeSet<&str>) -> bool {
        let changed = self.pages.get(document) != Some(&queries);
        self.pages.insert(document.to_owned(), queries);
        self.order.retain(|known| known != document);
        self.order.push_back(document.to_owned());
        while self.order.len() > MAX_DOCUMENTS {
            let oldest = self
                .order
                .iter()
                .position(|known| known != document && !shown.contains(known.as_str()));
            let Some(removed) = oldest.and_then(|at| self.order.remove(at)) else {
                break;
            };
            self.pages.remove(&removed);
        }
        changed
    }

    /// Forgets every result; `false` when there was none.
    pub fn clear(&mut self) -> bool {
        let had = !self.pages.is_empty();
        self.pages.clear();
        self.order.clear();
        had
    }
}

/// The boundaries a footer names, of the footer's `own` read of the view's document and of
/// what Analyze Breakpoints found for it (`manual`; `None` when that does not count for the
/// view). An own read that is incomplete (stylesheets of another origin skipped, the cap hit)
/// or found nothing never replaces a manual result; of two that count, the one that saw more
/// boundaries is taken. The names and sources the manual analysis gave its boundaries are kept.
pub fn choose(
    own: Option<&PageQueries>,
    manual: Option<&[DetectedBreakpoint]>,
) -> Vec<DetectedBreakpoint> {
    let manual = manual.filter(|manual| !manual.is_empty());
    match (own, manual) {
        (None, None) => Vec::new(),
        (None, Some(manual)) => manual.to_vec(),
        (Some(own), None) => own.detected.clone(),
        (Some(own), Some(manual)) => {
            if !own.complete() || own.detected.len() < manual.len() {
                return manual.to_vec();
            }
            own.detected
                .iter()
                .map(|found| {
                    let known = manual
                        .iter()
                        .find(|known| known.width == found.width && known.kind == found.kind);
                    let mut found = found.clone();
                    if let Some(known) = known {
                        found.label = found.label.or_else(|| known.label.clone());
                        found.source = found.source.or_else(|| known.source.clone());
                    }
                    found
                })
                .collect()
        }
    }
}

/// Whether the footer's own read is what [`choose`] names - and so whether what it left out is
/// worth a word in the tooltip.
pub fn own_counts(own: Option<&PageQueries>, manual: Option<&[DetectedBreakpoint]>) -> bool {
    let manual = manual.filter(|manual| !manual.is_empty());
    match (own, manual) {
        (Some(_), None) => true,
        (Some(own), Some(manual)) => own.complete() && own.detected.len() >= manual.len(),
        (None, _) => false,
    }
}

#[cfg(test)]
#[path = "results_tests.rs"]
mod tests;
