//! What MCP's resources read of the last check run and the last screenshots (RB-095-01):
//! `rbrowser://findings/latest` the run the findings panel got ([`crate::checks`]), and
//! `rbrowser://screenshots/latest` the captures [`crate::capture::latest`] kept, each file read
//! by its index only - a URI never names a path.

use std::fs;
use std::io::Read;
use std::path::Path;

use rb_mcp::ToolFailure;
use serde_json::{Value, json};

/// The first bytes of every PNG.
pub(crate) const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// The last check run as `{ run, messages }`; `None` before the first.
pub fn findings() -> Option<Value> {
    let (run, messages) = crate::checks::last_run()?;
    Some(json!({ "run": run, "messages": messages }))
}

/// The last screenshots saved in the project `root`, without image data.
pub fn screenshots(root: Option<&Path>) -> Option<Value> {
    let answer = crate::capture::latest::recall(root?)?;
    serde_json::to_value(answer).ok()
}

/// The PNG of file `index` of the last screenshots of `root` (the shots with a file in their
/// order, the overview last) when it has at most `max_bytes`.
///
/// # Errors
///
/// `screenshots.none` without such a file (none saved, the index past the end, the file gone,
/// a link or no PNG in its place - a link is never followed), `answer.too_large` over
/// `max_bytes`.
pub fn screenshot_png(
    root: Option<&Path>,
    index: usize,
    max_bytes: usize,
) -> Result<Vec<u8>, ToolFailure> {
    let answer = root
        .and_then(crate::capture::latest::recall)
        .ok_or_else(|| none("no screenshot was saved yet: take one with screenshot"))?;
    let path = answer
        .paths()
        .get(index)
        .map(|path| Path::new(path).to_path_buf())
        .ok_or_else(|| {
            none(&format!(
                "rbrowser://screenshots/latest has no file {index}"
            ))
        })?;
    let gone = || none("the screenshot file is gone; take a new one with screenshot");
    let metadata = fs::symlink_metadata(&path).map_err(|_| gone())?;
    if !metadata.file_type().is_file() {
        return Err(none(
            "the screenshot file is no longer a plain file; take a new one with screenshot",
        ));
    }
    let size = metadata.len();
    let too_large = || {
        ToolFailure::new(
            "answer.too_large",
            format!(
                "the image has more than mcp.maxAnswerBytes allows ({max_bytes} bytes before \
                 base64); open its path instead"
            ),
        )
        .with_setting("mcp.maxAnswerBytes")
    };
    if !usize::try_from(size).is_ok_and(|size| size <= max_bytes) {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    // The file may have grown since its size was read: one byte over the limit says so.
    fs::File::open(path)
        .and_then(|file| {
            file.take(
                u64::try_from(max_bytes)
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )
            .read_to_end(&mut bytes)
        })
        .map_err(|_| gone())?;
    if bytes.len() > max_bytes {
        return Err(too_large());
    }
    if !bytes.starts_with(&PNG_SIGNATURE) {
        return Err(none(
            "the screenshot file is no PNG; take a new one with screenshot",
        ));
    }
    Ok(bytes)
}

fn none(message: &str) -> ToolFailure {
    ToolFailure::new("screenshots.none", message)
}

#[cfg(test)]
#[path = "latest_tests.rs"]
mod tests;
