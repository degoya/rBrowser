//! What one answer of the component may carry (RB-090-17). The host refuses an answer to a
//! panel's call over [`MESSAGE_BYTES`] (`plugins.error.too_large`), so the lists the scans and
//! the tab order answer - which come from the page and grow with it - share a byte budget, and
//! the answer keeps [`RESERVE_BYTES`] free for the rest (the overlays' state, the counts). Every
//! list keeps its whole count beside the items it shows (`count`, `shown`).

use serde::Serialize;

use crate::tab_compare::MAX_DIFF_BYTES;

/// The largest message a panel or page script takes, as JSON text in bytes: the host's
/// `rb_plugin_host::MAX_MESSAGE_BYTES`.
pub const MESSAGE_BYTES: usize = 64 * 1024;
/// What an answer keeps free of listed items.
pub const RESERVE_BYTES: usize = 16 * 1024;
/// The bytes the hits of a checking overlay's scan take in one answer, every view together.
pub const SCAN_BYTES: usize = MESSAGE_BYTES - RESERVE_BYTES;
/// The bytes the tab order's stops take in one answer, every view together; its two lists of
/// differences take at most [`MAX_DIFF_BYTES`] each beside them.
pub const TAB_BYTES: usize = MESSAGE_BYTES - RESERVE_BYTES - 2 * MAX_DIFF_BYTES;

/// How many of the first items of each of `lists` fit in `budget` bytes of JSON text together,
/// at most `most` of each. The lists take one item each in turn, so every list gets its share; a
/// list stops at its first item that no longer fits.
#[must_use]
pub fn share<T: Serialize>(lists: &[&[T]], most: usize, budget: usize) -> Vec<usize> {
    let mut taken = vec![0_usize; lists.len()];
    let mut open: Vec<bool> = lists.iter().map(|list| !list.is_empty()).collect();
    let mut used = 0_usize;
    while open.iter().any(|open| *open) {
        for (index, list) in lists.iter().enumerate() {
            if !open[index] {
                continue;
            }
            let next = list.get(taken[index]).filter(|_| taken[index] < most);
            // The item and the comma before it.
            let size = next
                .and_then(|item| serde_json::to_string(item).ok())
                .map_or(usize::MAX, |text| text.len().saturating_add(1));
            if used.saturating_add(size) > budget {
                open[index] = false;
                continue;
            }
            used += size;
            taken[index] += 1;
        }
    }
    taken
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
