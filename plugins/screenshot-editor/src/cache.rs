//! An image's chunks, cut once per file version (Charge 9 review M3, the approach of
//! image-overlay's `cache.rs`).
//!
//! The panel loads an image in chunks of [`CHUNK_BYTES`] (`load`). `project.read` reads a file
//! only whole, so answering each chunk from the image meant reading and fingerprinting the whole
//! file again: an 8 MiB full-page screenshot took some 175 full reads. So the first chunk
//! (`offset` 0) cuts the version it read into
//! `.rbrowser/out/screenshot-editor/chunks/<hash>/<index>.b64` (the answer's base64 text) and
//! writes `bytes` (the file's length) last; a later chunk that names the `hash` of the first is
//! then one small read. The folder is named by the fingerprint the panel holds, so a version
//! never changes under a panel that is loading it.
//!
//! Whatever does not fit - no `bytes`, a chunk file of the wrong length (a write still under
//! way), a hash that is no fingerprint, an offset between chunks - is answered from the image as
//! before, which also reports a changed file. The folder lies in `out/`, which
//! `.rbrowser/.gitignore` keeps out of git; at most [`MAX_VERSIONS`] versions are cut (the plugin
//! cannot delete), beyond them the chunks come from the image. Deleting the folder clears it.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rb_plugin_api::PluginHost;

use crate::paths::OWN_ROOT;
use crate::tools::CHUNK_BYTES;

/// The cut versions, relative to the plugin's out folder (`out.write`).
pub const CHUNKS_DIR: &str = "chunks";
/// The most versions cut; beyond them every chunk is read from the image.
pub const MAX_VERSIONS: usize = 32;
/// The file a cut version ends with: the image's length in bytes.
const LENGTH_FILE: &str = "bytes";
/// The largest image cut (`project.read`'s limit).
const MAX_BYTES: usize = rb_plugin_api::MAX_IO_BYTES;

/// What the cache holds for one chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cached {
    /// The chunk's base64 text and the image's length.
    Data { data: String, bytes: usize },
    /// The version is cut and ends before the offset.
    PastEnd,
    /// Not cut (or unusable): read the image.
    Missing,
}

/// Whether `hash` is a fingerprint (`tools::fingerprint`: 16 lower-case hex digits) and so a
/// safe folder name.
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
    host.project_read(&format!("{OWN_ROOT}{}", out_path(hash, name)))
        .ok()
}

/// The length of the version `hash`, when it is cut completely.
fn cut_length(host: &mut dyn PluginHost, hash: &str) -> Option<usize> {
    let text = String::from_utf8(read(host, hash, LENGTH_FILE)?).ok()?;
    text.trim()
        .parse::<usize>()
        .ok()
        .filter(|length| (1..=MAX_BYTES).contains(length))
}

/// Cuts `bytes` (the version `hash`) into chunk files, unless it fits one chunk, is cut already
/// or [`MAX_VERSIONS`] are. A failure is logged: the chunks then come from the image.
pub fn prepare(host: &mut dyn PluginHost, bytes: &[u8], hash: &str) {
    if bytes.len() <= CHUNK_BYTES || !hash_ok(hash) || cut_length(host, hash).is_some() {
        return;
    }
    let versions = host
        .project_list(&format!("{OWN_ROOT}{CHUNKS_DIR}"))
        .map_or(0, |names| names.len());
    if versions >= MAX_VERSIONS {
        host.log(
            "warn",
            &format!("{MAX_VERSIONS} image versions cut; delete {OWN_ROOT}{CHUNKS_DIR}/"),
        );
        return;
    }
    for (index, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
        let name = format!("{index}.b64");
        let text = STANDARD.encode(chunk);
        if let Err(failure) = host.out_write(&out_path(hash, &name), text.as_bytes()) {
            let code = failure.code.unwrap_or_default();
            host.log("warn", &format!("image chunks not written: {code}"));
            return;
        }
    }
    let length = bytes.len().to_string();
    if let Err(failure) = host.out_write(&out_path(hash, LENGTH_FILE), length.as_bytes()) {
        let code = failure.code.unwrap_or_default();
        host.log("warn", &format!("image chunks not written: {code}"));
    }
}

/// The chunk at `offset` of the version `hash`, as far as the cache holds it.
pub fn chunk(host: &mut dyn PluginHost, hash: &str, offset: usize) -> Cached {
    if !hash_ok(hash) {
        return Cached::Missing;
    }
    let Some(length) = cut_length(host, hash) else {
        return Cached::Missing;
    };
    if offset > length {
        return Cached::PastEnd;
    }
    if offset == length || offset % CHUNK_BYTES != 0 {
        return Cached::Missing;
    }
    let index = offset / CHUNK_BYTES;
    let expected = (length - offset).min(CHUNK_BYTES).div_ceil(3) * 4;
    match read(host, hash, &format!("{index}.b64")).map(String::from_utf8) {
        Some(Ok(data)) if data.len() == expected && data.bytes().all(is_base64) => Cached::Data {
            data,
            bytes: length,
        },
        _ => Cached::Missing,
    }
}

fn is_base64(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
