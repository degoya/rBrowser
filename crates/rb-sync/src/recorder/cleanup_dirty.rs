//! Which steps changed (RB-090-89): the first place a step was added, changed or taken out since
//! the recorder last handed the changes over ([`super::super::record::Recorder::take_changed`]),
//! so the Recorder panel gets only the steps from there while a recording runs.

use super::{Builder, Entry};

impl Builder {
    /// Step `at` and the ones after it changed.
    pub(crate) fn touch(&mut self, at: usize) {
        self.dirty = Some(self.dirty.map_or(at, |dirty| dirty.min(at)));
    }

    /// Adds `entry` at the end.
    pub(super) fn push_entry(&mut self, entry: Entry) {
        self.touch(self.entries.len());
        self.entries.push(entry);
    }

    /// The last step, to change it.
    pub(super) fn last_entry(&mut self) -> Option<&mut Entry> {
        let last = self.entries.len().checked_sub(1)?;
        self.touch(last);
        self.entries.last_mut()
    }

    /// The last step, to look at it and maybe change it; the caller [`Self::touch`]es it when it
    /// does.
    pub(super) fn last_entry_untouched(&mut self) -> Option<&mut Entry> {
        self.entries.last_mut()
    }

    /// Takes the last step out.
    pub(super) fn pop_entry(&mut self) -> Option<Entry> {
        let entry = self.entries.pop()?;
        self.touch(self.entries.len());
        Some(entry)
    }

    /// The first place that changed since the last call; `None` when no step did.
    pub(crate) fn take_dirty(&mut self) -> Option<usize> {
        self.dirty.take()
    }
}
