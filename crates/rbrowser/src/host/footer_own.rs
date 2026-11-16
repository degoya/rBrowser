//! The host's half of the footer's own reads (`ui.viewFooter.analyze`, RB-080-47,
//! `crate::view_footer::auto`): where a read starts, where its result is kept and when the
//! kept results go. The results are the host's, beside what Analyze Breakpoints found
//! (`shell.detected`), which nothing here touches.

use std::collections::BTreeSet;

use rb_config::EngineKind;
use rb_engine::ViewId;

use super::super::Host;
use super::super::state::HostState;
use crate::view_footer::auto::AutoState;
use crate::view_footer::read::config_mark;
use crate::view_footer::{PageQueries, document_of};

/// What a read of a view starts from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadStart {
    /// The document the host knows for the view: the key its result is kept under.
    pub document: String,
    /// The number the result must still carry to be kept.
    pub generation: u64,
    /// The mark of the config the read is analysed with.
    pub config_mark: u64,
}

/// Notes the config in effect: when what an analysis depends on changed, the kept results go,
/// the reads under way are not kept and the views are to be read again.
pub(super) fn note_config(state: &mut HostState) {
    let mark = config_mark(&state.config);
    let footer = &mut state.footer;
    if footer
        .config_mark
        .replace(mark)
        .is_some_and(|before| before != mark)
    {
        footer.own.clear();
        footer.generation += 1;
        footer.stale = true;
    }
}

impl Host {
    /// Works on the waits, the switch and the reads under way of the footer's reads.
    pub fn footer_auto<T>(&self, action: impl FnOnce(&mut AutoState) -> T) -> T {
        action(&mut self.state().footer.auto)
    }

    /// Where a read of `view` starts; `None` for a view that is gone, shows no page yet or is
    /// not Chromium's (the script runs over CDP; the other engines' views take the result of
    /// their document).
    pub fn footer_read_start(&self, view: ViewId) -> Option<ReadStart> {
        let state = self.state();
        let entry = state.views.get(&view)?;
        let engine = entry
            .breakpoint
            .as_ref()
            .map_or(EngineKind::Chromium, |breakpoint| breakpoint.engine);
        if engine != EngineKind::Chromium || entry.url.is_empty() {
            return None;
        }
        Some(ReadStart {
            document: document_of(&entry.url).to_owned(),
            generation: state.footer.generation,
            config_mark: state
                .footer
                .config_mark
                .unwrap_or_else(|| config_mark(&state.config)),
        })
    }

    /// The footer read `document` in a view: the views that show it name these queries, beside
    /// what Analyze Breakpoints found. `false` when the result is not kept: the read started
    /// under another `generation` - the setting was switched off or the config changed since.
    pub fn footer_page_read(&self, document: &str, queries: PageQueries, generation: u64) -> bool {
        let changed = {
            let mut state = self.state();
            let footer = &mut state.footer;
            if footer.generation != generation {
                return false;
            }
            let shown: BTreeSet<&str> = footer.documents.values().map(String::as_str).collect();
            footer.own.put(document, queries, &shown)
        };
        if changed {
            self.refresh_footers();
        }
        true
    }

    /// The signature of the read kept for `document`; `None` without one.
    pub fn footer_result_signature(&self, document: &str) -> Option<u64> {
        let state = self.state();
        state.footer.own.get(document).map(|own| own.signature)
    }

    /// `ui.viewFooter.analyze` was switched off: the footer's own results go, a read under way
    /// is not kept, and the footers name the ends of their ranges (or what Analyze Breakpoints
    /// found).
    pub fn footer_results_clear(&self) {
        let had = {
            let mut state = self.state();
            state.footer.generation += 1;
            state.footer.own.clear()
        };
        if had {
            self.refresh_footers();
        }
    }

    /// Whether a changed config dropped the results since this was asked last: the views are
    /// read again then.
    pub fn footer_take_stale(&self) -> bool {
        std::mem::take(&mut self.state().footer.stale)
    }
}
