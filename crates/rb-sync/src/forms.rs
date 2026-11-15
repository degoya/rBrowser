//! Form sync (RB-020-05) on the protocol's side: what MCP and the recorder may see of a typed
//! value, and which `change` carries nothing to set.
//!
//! A password field's value travels to the other views in clear - they need it - marked
//! `sensitive`. Everything that shows events to a person or a tool (MCP answers, the recorder of
//! RB-020-*, a debug dump) goes through [`redact`] first; `Debug` masks every typed value anyway
//! (`event.rs`).

use crate::event::{ChangeData, KeyData, SyncEvent};

/// What a sensitive value is shown as.
pub const MASK: &str = "********";

/// Whether `event` carries a sensitive value (a password field's input, change or key).
pub fn is_sensitive(event: &SyncEvent) -> bool {
    match event {
        SyncEvent::Input(input) => input.sensitive,
        SyncEvent::Change(change) => change.sensitive,
        SyncEvent::Keydown(key) | SyncEvent::Keyup(key) => key.sensitive,
        _ => false,
    }
}

/// `event` as MCP and the recorder may show it: a sensitive value and its typed data masked, a
/// sensitive key's `key`, `code` and `keyCode` too; everything else - the target included -
/// unchanged.
pub fn redact(event: &SyncEvent) -> SyncEvent {
    let mask = |value: &Option<String>| value.as_ref().map(|_| MASK.to_owned());
    match event {
        SyncEvent::Input(input) if input.sensitive => {
            let mut shown = input.clone();
            shown.data = mask(&input.data);
            shown.value = mask(&input.value);
            SyncEvent::Input(shown)
        }
        SyncEvent::Change(change) if change.sensitive => {
            let mut shown = change.clone();
            shown.value = mask(&change.value);
            SyncEvent::Change(shown)
        }
        SyncEvent::Keydown(key) if key.sensitive => SyncEvent::Keydown(masked_key(key)),
        SyncEvent::Keyup(key) if key.sensitive => SyncEvent::Keyup(masked_key(key)),
        other => other.clone(),
    }
}

/// A sensitive key as [`redact`] shows it.
fn masked_key(key: &KeyData) -> KeyData {
    KeyData {
        key: MASK.to_owned(),
        code: MASK.to_owned(),
        key_code: 0,
        ..key.clone()
    }
}

/// Whether `change` carries no state at all: a file input's (its value, a local path, is never
/// captured). The other views skip it - no desync.
pub fn is_file_skip(change: &ChangeData) -> bool {
    change.value.is_none() && change.checked.is_none() && change.selected.is_none()
}

#[cfg(test)]
#[path = "forms_tests.rs"]
mod tests;
