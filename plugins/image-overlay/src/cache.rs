//! A design's chunks, cut once per file version (Charge 8 review M3).
//!
//! The page script fetches a design in chunks of [`CHUNK_BYTES`] - as much as fits a page
//! script's 64 KiB answer as base64. `project.read` reads a file only whole, so answering each
//! chunk from the design meant reading, checking and fingerprinting the whole file again: a
//! 4 MiB design took 88 full reads per view. So `init` cuts the version it chose once into
//! `.rbrowser/out/image-overlay/chunks/<hash>/<index>.b64` (the answer's base64 text) and writes
//! `bytes` (the file's length) last; a chunk is then one small read. The directory is named by
//! the fingerprint the page holds, so a version never changes under a page that is fetching it.
//!
//! Whatever does not fit - no `bytes`, a chunk file of the wrong length (a write still under
//! way), a hash that is no fingerprint - is answered from the design as before, which also
//! reports a changed file. The folder lies in `out/`, which `.rbrowser/.gitignore` keeps out of
//! git; at most [`MAX_VERSIONS`] versions are cut (the plugin cannot delete), beyond them the
//! chunks come from the design. Deleting the folder clears it.

use rb_plugin_api::PluginHost;

use crate::CHUNK_BYTES;
use crate::image::{MAX_IMAGE_BYTES, base64};
use crate::path::DesignPath;
use crate::select::read_checked;

/// The plugin's out folder, relative to the project (`project.read`).
pub const OUT_PROJECT_DIR: &str = ".rbrowser/out/image-overlay";
/// The cut versions, relative to the out folder (`out.write`).
pub const CHUNKS_DIR: &str = "chunks";
/// The most versions cut; beyond them every chunk is read from the design.
pub const MAX_VERSIONS: usize = 32;
/// The file a cut version ends with: the design's length in bytes.
const LENGTH_FILE: &str = "bytes";

/// What the cache holds for one chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cached {
    /// The chunk's base64 text.
    Data(String),
    /// The version is cut and has no chunk `index`.
    PastEnd,
    /// Not cut (or unusable): read the design.
    Missing,
}

/// Whether `hash` is a fingerprint ([`crate::image::fingerprint`]: 16 lower-case hex digits) and
/// so a safe folder name.
#[must_use]
pub fn hash_ok(hash: &str) -> bool {
    hash.len() == 16
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn out_path(hash: &str, name: &str) -> String {
    format!("{CHUNKS_DIR}/{hash}/{name}")
}

fn read(host: &mut dyn PluginHost, hash: &str, name: &str) -> Option<Vec<u8>> {
    host.project_read(&format!("{OUT_PROJECT_DIR}/{}", out_path(hash, name)))
        .ok()
}

/// The length of the design version `hash`, when it is cut completely.
fn cut_length(host: &mut dyn PluginHost, hash: &str) -> Option<usize> {
    let text = String::from_utf8(read(host, hash, LENGTH_FILE)?).ok()?;
    text.trim()
        .parse::<usize>()
        .ok()
        .filter(|length| (1..=MAX_IMAGE_BYTES).contains(length))
}

/// Cuts the design `path` with the fingerprint `hash` into chunk files, unless that version is
/// cut already or [`MAX_VERSIONS`] are. A failure is logged: the chunks then come from the design.
pub fn prepare(host: &mut dyn PluginHost, path: &DesignPath, hash: &str) {
    if !hash_ok(hash) || cut_length(host, hash).is_some() {
        return;
    }
    let versions = host
        .project_list(&format!("{OUT_PROJECT_DIR}/{CHUNKS_DIR}"))
        .map_or(0, |names| names.len());
    if versions >= MAX_VERSIONS {
        host.log(
            "warn",
            &format!("{MAX_VERSIONS} design versions cut; delete {OUT_PROJECT_DIR}/{CHUNKS_DIR}/"),
        );
        return;
    }
    let Ok(bytes) = read_checked(host, path, hash) else {
        return;
    };
    for (index, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
        let name = format!("{index}.b64");
        if let Err(failure) = host.out_write(&out_path(hash, &name), base64(chunk).as_bytes()) {
            let code = failure.code.unwrap_or_default();
            host.log("warn", &format!("design chunks not written: {code}"));
            return;
        }
    }
    let length = bytes.len().to_string();
    if let Err(failure) = host.out_write(&out_path(hash, LENGTH_FILE), length.as_bytes()) {
        let code = failure.code.unwrap_or_default();
        host.log("warn", &format!("design chunks not written: {code}"));
    }
}

/// Chunk `index` of the design version `hash`, as far as the cache holds it.
pub fn chunk(host: &mut dyn PluginHost, hash: &str, index: usize) -> Cached {
    if !hash_ok(hash) {
        return Cached::Missing;
    }
    let Some(length) = cut_length(host, hash) else {
        return Cached::Missing;
    };
    let Some(start) = index
        .checked_mul(CHUNK_BYTES)
        .filter(|start| *start < length)
    else {
        return Cached::PastEnd;
    };
    let expected = (length - start).min(CHUNK_BYTES).div_ceil(3) * 4;
    match read(host, hash, &format!("{index}.b64")).map(String::from_utf8) {
        Some(Ok(text)) if text.len() == expected && text.bytes().all(is_base64) => {
            Cached::Data(text)
        }
        _ => Cached::Missing,
    }
}

fn is_base64(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
