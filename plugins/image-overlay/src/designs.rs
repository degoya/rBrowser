//! The MCP tool `imageoverlay_designs` (RB-080-74), which the panel calls for its choices and
//! previews.
//!
//! - No argument: the designs in `.rbrowser/designs/` (top level, by name, at most
//!   [`MAX_LISTED`]; `more` when the folder holds more), each `{name, path, mime, bytes, hash,
//!   width, height, scale, chunks}` - or `{name, path, problem}` for a file that is no usable
//!   design; `trusted` - whether a design can be added (an open project in a trusted workspace,
//!   [`crate::store::workspace_trusted`]); `chunkBytes`.
//! - `{"path", "hash", "chunk"}`: chunk `chunk` of that design as base64 `data`, for the file
//!   whose fingerprint is `hash` (`changed` once it is another) - the page script's `chunk`, so
//!   the panel draws a small preview from the bytes the overlay checked. The version is cut into
//!   chunk files once ([`crate::cache`]).
//! - `{"thumb": "<hash>"}`: the preview the panel drew of that version, or `data: null`;
//!   `{"thumb": "<hash>", "data": "<base64 PNG>"}` keeps one ([`crate::thumbs`]) - so a design
//!   is fetched whole once for its preview, not at every opening of the panel.
//!
//! Adding a design is no tool: it opens the native file dialog (`add_design`, a command).

use rb_plugin_api::{Failure, PluginHost};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::page::{chunk_of, meta};
use crate::path::{DESIGNS_PROJECT_DIR, DesignPath};
use crate::select::{chosen, design_path, read_design};
use crate::state::effective;
use crate::{CHUNK_BYTES, bad_args, cache, store, thumbs};

/// The most designs one answer lists: each entry is about 250 bytes, so the answer stays well
/// below a panel's 64 KiB message.
pub const MAX_LISTED: usize = 64;

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct DesignsArgs {
    path: Option<String>,
    hash: Option<String>,
    chunk: Option<usize>,
    thumb: Option<String>,
    data: Option<String>,
    target: Option<Value>,
}

/// Runs the tool.
///
/// # Errors
///
/// `bad_args` for arguments that do not read, or a chunk request without all three of `path`,
/// `hash` and `chunk`; a chunk's problem (`path_outside`, `changed`, ...).
pub fn run(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args: DesignsArgs = if args.trim().is_empty() {
        DesignsArgs::default()
    } else {
        serde_json::from_str(args).map_err(|_| bad_args("arguments"))?
    };
    // `target` is MCP's; the list is the project's, the same for every view.
    let _ = args.target;
    if let Some(hash) = args.thumb {
        if args.path.is_some() || args.hash.is_some() || args.chunk.is_some() {
            return Err(bad_args("thumb"));
        }
        let answer = match args.data {
            None => thumbs::read(host, &hash)?,
            Some(data) => thumbs::store(host, &hash, &data)?,
        };
        return Ok(answer.to_string());
    }
    if args.data.is_some() {
        return Err(bad_args("data"));
    }
    match (args.path, args.hash, args.chunk) {
        (None, None, None) => Ok(list(host).to_string()),
        (Some(path), Some(hash), Some(index)) => {
            let path = design_path(&path).map_err(|problem| problem.failure())?;
            if index == 0 {
                cache::prepare(host, &path, &hash);
            }
            chunk_of(host, &path, &hash, index).map(|answer| answer.to_string())
        }
        _ => Err(bad_args("chunk")),
    }
}

fn list(host: &mut dyn PluginHost) -> Value {
    let auto_scale = effective(&store::shared(host), &store::overrides(host, None)).auto_scale_dpr;
    let mut names: Vec<String> = host
        .project_list(DESIGNS_PROJECT_DIR)
        .unwrap_or_default()
        .into_iter()
        .filter(|name| DesignPath::listed(name).is_some())
        .collect();
    names.sort();
    let more = names.len() > MAX_LISTED;
    let designs: Vec<Value> = names
        .into_iter()
        .take(MAX_LISTED)
        .filter_map(|name| {
            let path = DesignPath::listed(&name)?;
            Some(match read_design(host, &path) {
                Ok((bytes, info)) => {
                    let mut entry = meta(&chosen(path, &bytes, info, auto_scale));
                    entry["name"] = json!(name);
                    entry
                }
                Err(problem) => json!({
                    "name": name,
                    "path": path.as_str(),
                    "problem": problem.json(),
                }),
            })
        })
        .collect();
    json!({
        "designs": designs,
        "more": more,
        "trusted": store::workspace_trusted(host),
        "chunkBytes": CHUNK_BYTES,
    })
}

#[cfg(test)]
#[path = "designs_tests.rs"]
mod tests;
