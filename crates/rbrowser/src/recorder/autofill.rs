//! The form autofill's fills in a recording (RB-090-78, `rb_sync::recorder::autofill`). The
//! `form-autofill` plugin fills by script, which the sync agent does not capture - it records
//! trusted input only, so a page's own scripts never make steps. After a fill the plugin names,
//! per view, the fields it set in `host` posts `{"action": "record_fill", "view": <id>, "fields":
//! [...]}` ([`crate::plugins::host_actions`] runs them for that bundled plugin only, from its
//! commands and tools, never from a page request); [`record`] hands each to the window's recorder,
//! which takes them as steps while a recording runs with `automation.recorder.recordAutofill`.
//! Without a recording nothing happens, and a malformed post or field is left out.

use rb_engine::ViewId;
use rb_sync::recorder::autofill::MAX_FIELDS;
use rb_sync::recorder::{AutofillField, AutofillView};
use serde_json::Value;

use crate::control::Core;

/// The host action of the `form-autofill` plugin that reports a fill.
pub const RECORD_FILL: &str = "record_fill";

/// The fill a `record_fill` post names: its view and the fields that hold, at most
/// [`MAX_FIELDS`]; `None` without a view or a field list.
#[must_use]
pub fn parse(message: &Value) -> Option<AutofillView> {
    let view = u32::try_from(message.get("view")?.as_u64()?).ok()?;
    let fields = message
        .get("fields")?
        .as_array()?
        .iter()
        .take(MAX_FIELDS)
        .filter_map(|field| serde_json::from_value::<AutofillField>(field.clone()).ok())
        .collect();
    Some(AutofillView {
        view: ViewId(view),
        fields,
    })
}

/// Records the fill of the post `message` in `core`'s window's recording, when one runs.
pub fn record(core: &Core, message: &Value) {
    let Some(fill) = parse(message) else {
        tracing::debug!("a form autofill report that does not hold; not recorded");
        return;
    };
    // No recorder was made: no recording ever ran in this window.
    if let Some(recorder) = super::for_host(core.host()) {
        recorder.on_autofill(&fill);
    }
}

#[cfg(test)]
#[path = "autofill_tests.rs"]
mod tests;
