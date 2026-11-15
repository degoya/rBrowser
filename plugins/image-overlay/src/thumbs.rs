//! The panel's previews of the designs (RB-080-74, review C7): a small PNG per design version,
//! kept in `.rbrowser/out/image-overlay/thumbs/<fingerprint>.png`.
//!
//! The component cannot decode JPEG or WebP, and a 16-megapixel design would fill its memory;
//! the panel's browser decodes every type. So the panel draws a preview once - from the design's
//! chunks - and hands it here; from then on, in every session, the preview is one small read
//! instead of the whole design. The name is the design's fingerprint, so a changed design gets a
//! new preview and an unchanged one keeps its own. Only a PNG of at most [`MAX_SIDE`] px a side
//! and [`MAX_BYTES`] is kept, at most [`MAX_THUMBS`] of them; a file there that is no such PNG
//! reads as none. The folder lies in `out/`, which `.rbrowser/.gitignore` keeps out of git;
//! deleting it clears the previews.

use rb_plugin_api::{Failure, PluginHost};
use serde_json::{Value, json};

use crate::bad_args;
use crate::cache::{OUT_PROJECT_DIR, hash_ok};
use crate::image::{Kind, base64, sniff, unbase64};

/// The previews' folder, relative to the out folder (`out.write`).
pub const THUMBS_DIR: &str = "thumbs";
/// The longest side of a preview in pixels.
pub const MAX_SIDE: u32 = 256;
/// The largest preview file: as base64 in the panel's call it stays well below the 64 KiB message.
pub const MAX_BYTES: usize = 32 * 1024;
/// The most previews kept; beyond them a new one is drawn each session.
pub const MAX_THUMBS: usize = 256;

fn file_name(hash: &str) -> String {
    format!("{hash}.png")
}

/// Whether `bytes` is a preview this module keeps.
fn usable(bytes: &[u8]) -> bool {
    bytes.len() <= MAX_BYTES
        && sniff(bytes, Kind::Png)
            .is_ok_and(|info| info.width <= MAX_SIDE && info.height <= MAX_SIDE)
}

/// The preview of the design version `hash`: `{"thumb", "data"}`, `data` the PNG as base64 or
/// `null` when there is none (yet).
///
/// # Errors
///
/// `bad_args` for a `hash` that is no fingerprint.
pub fn read(host: &mut dyn PluginHost, hash: &str) -> Result<Value, Failure> {
    if !hash_ok(hash) {
        return Err(bad_args("thumb"));
    }
    let path = format!("{OUT_PROJECT_DIR}/{THUMBS_DIR}/{}", file_name(hash));
    let data = host
        .project_read(&path)
        .ok()
        .filter(|bytes| usable(bytes))
        .map(|bytes| base64(&bytes));
    Ok(json!({ "thumb": hash, "data": data }))
}

/// Keeps `data` (base64 of a PNG) as the preview of the design version `hash`: `{"thumb",
/// "saved"}`, `saved` false when [`MAX_THUMBS`] are kept already or the write failed (logged).
///
/// # Errors
///
/// `bad_args` for a `hash` that is no fingerprint, or `data` that is no small PNG.
pub fn store(host: &mut dyn PluginHost, hash: &str, data: &str) -> Result<Value, Failure> {
    if !hash_ok(hash) {
        return Err(bad_args("thumb"));
    }
    if data.len() > MAX_BYTES.div_ceil(3) * 4 {
        return Err(bad_args("data"));
    }
    let bytes = unbase64(data)
        .filter(|bytes| usable(bytes))
        .ok_or_else(|| bad_args("data"))?;
    let name = file_name(hash);
    let kept = host
        .project_list(&format!("{OUT_PROJECT_DIR}/{THUMBS_DIR}"))
        .unwrap_or_default();
    if kept.len() >= MAX_THUMBS && !kept.contains(&name) {
        host.log(
            "info",
            &format!("{MAX_THUMBS} design previews kept; no more"),
        );
        return Ok(json!({ "thumb": hash, "saved": false }));
    }
    let saved = match host.out_write(&format!("{THUMBS_DIR}/{name}"), &bytes) {
        Ok(()) => true,
        Err(failure) => {
            let code = failure.code.unwrap_or_default();
            host.log("warn", &format!("design preview not written: {code}"));
            false
        }
    };
    Ok(json!({ "thumb": hash, "saved": saved }))
}

#[cfg(test)]
#[path = "thumbs_tests.rs"]
mod tests;
