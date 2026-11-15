//! The page script's requests (`call-tool("@page", {view, message})`, `docs/plugins.md`).
//!
//! - `{"op": "init", "width"?, "dpr"?, "overrides"?}`: the state, the overrides (the page keeps a
//!   copy), and the view's design as `image` - `{path, mime, bytes, hash, width, height, scale,
//!   chunks}` - or `null`, with the reason in `problem` (`{code, params}`) when the view's entry
//!   was refused. `width` and `dpr` are the page's own, used only for a view the host does not
//!   list.
//! - `{"op": "chunk", "path", "hash", "index"}`: the bytes `index * CHUNK_BYTES ..` of the design
//!   as base64 `data`, for the file whose fingerprint is `hash` (`changed` once it is another).
//!   Only a design below `.rbrowser/designs/` that is the image its name says can be read so.
//!   `init` cuts the version it answers into chunk files once ([`crate::cache`]), so a chunk is
//!   one small read, not the whole design again.
//! - `{"op": "split", "position", "overrides"?}`: the split handle let go at `position` per cent;
//!   the split is on in every view at that position.
//!
//! `overrides` is the page's copy, used only when the state file cannot be read (no project).

use rb_plugin_api::{Failure, PluginHost, ViewId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cache::{self, Cached};
use crate::image::base64;
use crate::path::DesignPath;
use crate::select::{Choice, Chosen, ViewFacts, choose, design_path, read_checked};
use crate::state::{Overrides, Split, effective, position_ok};
use crate::{CHUNK_BYTES, bad_args, changed, store};

/// The widest view a page may claim, in CSS px.
const MAX_WIDTH: f64 = 100_000.0;

#[derive(Deserialize)]
struct PageArgs {
    view: ViewId,
    message: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitRequest {
    op: String,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    dpr: Option<f64>,
    #[serde(default)]
    overrides: Option<Overrides>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkRequest {
    op: String,
    path: String,
    hash: String,
    index: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SplitRequest {
    op: String,
    position: f64,
    #[serde(default)]
    overrides: Option<Overrides>,
}

fn parse<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, Failure> {
    serde_json::from_value(value).map_err(|_| bad_args("message"))
}

/// How many chunks a design of `bytes` takes.
#[must_use]
pub fn chunks(bytes: usize) -> usize {
    bytes.div_ceil(CHUNK_BYTES)
}

/// A design as the page script and the panel get it.
#[must_use]
pub fn meta(chosen: &Chosen) -> Value {
    json!({
        "path": chosen.path.as_str(),
        "mime": chosen.info.kind.mime(),
        "bytes": chosen.bytes,
        "hash": chosen.hash,
        "width": chosen.info.width,
        "height": chosen.info.height,
        "scale": chosen.scale,
        "chunks": chunks(chosen.bytes),
    })
}

/// The view `id` as the host lists it; none when it does not (or there are no views to read).
pub fn listed_view(host: &mut dyn PluginHost, id: ViewId) -> Option<ViewFacts> {
    let views = host.views().ok()?;
    let view = views.into_iter().find(|view| view.id == id)?;
    Some(ViewFacts {
        breakpoint: view.breakpoint,
        width: f64::from(view.width),
        dpr: f64::from(view.dpr),
    })
}

/// The `image` and `problem` of a choice.
#[must_use]
pub fn choice_json(choice: &Choice) -> (Value, Value) {
    match choice {
        Choice::Image(chosen) => (meta(chosen), Value::Null),
        Choice::None => (Value::Null, Value::Null),
        Choice::Problem(problem) => (Value::Null, problem.json()),
    }
}

/// Answers a page script's request.
///
/// # Errors
///
/// The request is not one of the above or a value is out of range (`bad_args`); a chunk's
/// design is refused or changed (its problem's code).
pub fn handle(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let args: PageArgs = serde_json::from_str(args).map_err(|_| bad_args("arguments"))?;
    let op = args.message.get("op").and_then(Value::as_str).unwrap_or("");
    match op {
        "init" => init(host, args.view, parse(args.message)?),
        "chunk" => chunk(host, parse(args.message)?),
        "split" => split(host, parse(args.message)?),
        _ => Err(bad_args("op")),
    }
}

fn init(host: &mut dyn PluginHost, view: ViewId, request: InitRequest) -> Result<String, Failure> {
    debug_assert_eq!(request.op, "init");
    if request
        .width
        .is_some_and(|width| !width.is_finite() || !(1.0..=MAX_WIDTH).contains(&width))
    {
        return Err(bad_args("width"));
    }
    let dpr = request
        .dpr
        .filter(|dpr| dpr.is_finite() && (0.25..=8.0).contains(dpr))
        .unwrap_or(1.0);
    let overrides = store::overrides(host, request.overrides);
    let state = effective(&store::shared(host), &overrides);
    let facts = listed_view(host, view).or_else(|| {
        request.width.map(|width| ViewFacts {
            breakpoint: String::new(),
            width,
            dpr,
        })
    });
    let choice = match &facts {
        Some(facts) => choose(host, &state.designs, state.auto_scale_dpr, facts),
        None => Choice::None,
    };
    if let Choice::Image(chosen) = &choice {
        cache::prepare(host, &chosen.path, &chosen.hash);
    }
    let (image, problem) = choice_json(&choice);
    Ok(json!({
        "state": state,
        "overrides": overrides,
        "image": image,
        "problem": problem,
        "chunkBytes": CHUNK_BYTES,
    })
    .to_string())
}

fn chunk(host: &mut dyn PluginHost, request: ChunkRequest) -> Result<String, Failure> {
    debug_assert_eq!(request.op, "chunk");
    let path = design_path(&request.path).map_err(|problem| problem.failure())?;
    chunk_of(host, &path, &request.hash, request.index).map(|answer| answer.to_string())
}

/// Chunk `index` of the design `path` whose fingerprint is `hash`: `{"index", "data"}` from the
/// cut version ([`crate::cache`]), else from the design itself. The page script's `chunk` and
/// the panel's preview (`designs`).
///
/// # Errors
///
/// `bad_args` for an index past the end; the design's problem (`changed` for another version).
pub fn chunk_of(
    host: &mut dyn PluginHost,
    path: &DesignPath,
    hash: &str,
    index: usize,
) -> Result<Value, Failure> {
    match cache::chunk(host, hash, index) {
        Cached::Data(data) => return Ok(json!({ "index": index, "data": data })),
        Cached::PastEnd => return Err(bad_args("index")),
        Cached::Missing => {}
    }
    let bytes = read_checked(host, path, hash).map_err(|problem| problem.failure())?;
    let start = index
        .checked_mul(CHUNK_BYTES)
        .filter(|start| *start < bytes.len())
        .ok_or_else(|| bad_args("index"))?;
    let end = start.saturating_add(CHUNK_BYTES).min(bytes.len());
    Ok(json!({ "index": index, "data": base64(&bytes[start..end]) }))
}

fn split(host: &mut dyn PluginHost, request: SplitRequest) -> Result<String, Failure> {
    debug_assert_eq!(request.op, "split");
    if !position_ok(request.position) {
        return Err(bad_args("position"));
    }
    let mut overrides = store::overrides(host, request.overrides);
    let shared = store::shared(host);
    let before = effective(&shared, &overrides);
    overrides.split = Some(Split {
        on: true,
        position: request.position,
    });
    Ok(changed(host, &before, &shared, &overrides, json!({})))
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
