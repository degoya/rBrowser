//! Keys rBrowser no longer reads. A file that still sets one loads as before: the key is
//! dropped before the type check with one warning ([`RETIRED_KEY`]) that names it and says what
//! took its place, and the next save of the file leaves it out. No version step: the rest of
//! the file means what it meant.
//!
//! - `capture.video.format` (RB-090-52): recordings are always WebM (VP9, VP8 where the ffmpeg
//!   has no VP9), which plays in the Videos panel on every platform; an MP4 to share is
//!   exported in the Videos panel.
//!
//! A key that moved takes its value to its new place before that, without a warning
//! ([`move_keys`]); a value already at the new place wins:
//!
//! - `ui.cellLabel.vitals` -> `ui.viewFooter.vitals` (RB-090-71): the label strip's vitals became
//!   the vitals bar under the view; a person who switched them off keeps them off.

use serde_json::Value;

use crate::ConfigIssue;

/// The i18n key of a retired key's warning.
pub const RETIRED_KEY: &str = "config.warning.retired_key";

/// The retired keys as JSON pointers, each with what took its place.
const RETIRED: [(&str, &str); 1] = [(
    "/capture/video/format",
    "recordings are always WebM, which plays in the Videos panel; export an MP4 there to share \
     one",
)];

/// The moved keys as JSON pointers: the old place, the new one.
const MOVED: [(&str, &str); 1] = [("/ui/cellLabel/vitals", "/ui/viewFooter/vitals")];

/// Moves every moved key `value` sets to its new place, unless the new place has a value.
pub fn move_keys(value: &mut Value) {
    for (old, new) in MOVED {
        let (Some((old_parent, old_leaf)), Some((new_parent, new_leaf))) =
            (old.rsplit_once('/'), new.rsplit_once('/'))
        else {
            continue;
        };
        let Some(moved) = value
            .pointer_mut(old_parent)
            .and_then(Value::as_object_mut)
            .and_then(|object| object.remove(old_leaf))
        else {
            continue;
        };
        let Some(target) = object_at(value, new_parent) else {
            continue;
        };
        target.entry(new_leaf.to_owned()).or_insert(moved);
    }
}

/// The object at `pointer` in `value`, made where it is missing; `None` where something else
/// stands in the way.
fn object_at<'a>(
    value: &'a mut Value,
    pointer: &str,
) -> Option<&'a mut serde_json::Map<String, Value>> {
    let mut here = value;
    for segment in pointer.split('/').skip(1) {
        here = here
            .as_object_mut()?
            .entry(segment.to_owned())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
    }
    here.as_object_mut()
}

/// Removes every retired key `value` sets; one warning each.
pub fn strip_retired(value: &mut Value) -> Vec<ConfigIssue> {
    let mut warnings = Vec::new();
    for (pointer, instead) in RETIRED {
        let Some((parent, leaf)) = pointer.rsplit_once('/') else {
            continue;
        };
        let removed = value
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .and_then(|object| object.remove(leaf));
        if removed.is_some() {
            let key = pointer.trim_start_matches('/').replace('/', ".");
            warnings.push(ConfigIssue {
                path: pointer.to_owned(),
                key: RETIRED_KEY.to_owned(),
                message: format!("`{key}` is no longer read and was ignored: {instead}"),
            });
        }
    }
    warnings
}

#[cfg(test)]
#[path = "retired_tests.rs"]
mod tests;
