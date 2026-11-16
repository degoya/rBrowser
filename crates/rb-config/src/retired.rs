//! Keys rBrowser no longer reads. A file that still sets one loads as before: the key is
//! dropped before the type check with one warning ([`RETIRED_KEY`]) that names it and says what
//! took its place, and the next save of the file leaves it out. No version step: the rest of
//! the file means what it meant.
//!
//! - `capture.video.format` (RB-090-52): recordings are always WebM (VP9, VP8 where the ffmpeg
//!   has no VP9), which plays in the Videos panel on every platform; an MP4 to share is
//!   exported in the Videos panel.

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
