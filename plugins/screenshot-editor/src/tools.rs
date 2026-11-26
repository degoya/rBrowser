//! The plugin's tools - the panel's calls and MCP's `shot_annotate`, `shot_load`, `shot_list`:
//!
//! - [`annotate`] renders annotations onto an image and writes the result and the annotations
//!   below `.rbrowser/out/screenshot-editor/`, without any UI - band by band, so every image the
//!   capture writes (65535 rows, 256 megapixels) renders within the plugin's memory (RB-099-24);
//! - [`load`] hands an image to the panel in chunks of [`CHUNK_BYTES`] (base64, so an answer
//!   stays below a panel message's 64 KiB), the first with its size and saved annotations, held
//!   to [`ANSWER_BYTES`] as a whole; the first cuts the image once ([`crate::cache`]), so a later
//!   chunk naming its `hash` is one small read. An image too large for a canvas comes as its
//!   scaled preview (`scale` in the first answer, [`crate::preview`]);
//! - [`list`] names the images the editor can open.

use std::cell::RefCell;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rb_plugin_api::{Failure, FailureKind, MAX_IO_BYTES, PluginHost};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::band::{self, Bands};
use crate::cache::{self, Cached};
use crate::model::{Annotation, DOCUMENT_VERSION, Document, validate};
use crate::paths::{self, Format, Targets};
use crate::rows::{PngRows, RowSource};
use crate::stream::{OutFile, Shared};
use crate::{PLUGIN_ID, bad_args, codec, encode, files, preview, settings};

/// The bytes of the image one [`load`] answer carries.
pub const CHUNK_BYTES: usize = 47 * 1024;
/// The longest answer a panel takes: one panel message (`rb_plugin_host::MAX_MESSAGE_BYTES`,
/// held by `crates/rbrowser/src/plugins/runtime.rs` `bounded`), JSON text in UTF-8 bytes.
pub const ANSWER_BYTES: usize = 64 * 1024;

fn parse<'a, T: Deserialize<'a> + Default>(args: &'a str) -> Result<T, Failure> {
    if args.trim().is_empty() {
        return Ok(T::default());
    }
    serde_json::from_str(args).map_err(|error| {
        let reason = error.to_string();
        bad_args(reason.split(" at line").next().unwrap_or("arguments"))
    })
}

/// `shot_annotate`'s arguments; `target` is MCP's and means nothing here.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct AnnotateArgs {
    image: String,
    annotations: Vec<Annotation>,
    #[serde(default)]
    out: Option<String>,
    #[serde(default)]
    #[expect(dead_code, reason = "MCP adds it to every tool; an image has no view")]
    target: Option<Value>,
}

/// Renders `annotations` onto `image` and writes the edited image and the annotations.
pub fn annotate(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let AnnotateArgs {
        image,
        mut annotations,
        out,
        ..
    } = parse(args)?;
    let image = paths::source(&image)?;
    validate(&mut annotations)?;
    let settings = settings::read(host);
    let targets = match out.as_deref() {
        Some(out) => paths::custom(out)?,
        None => paths::mirror(&image, settings.format),
    };
    let rendered = render_file(host, &image, &annotations, &targets, settings.jpeg_quality)?;
    let document = Document {
        version: DOCUMENT_VERSION,
        image: image.clone(),
        width: rendered.source.0,
        height: rendered.source.1,
        annotations,
    };
    let text = serde_json::to_vec_pretty(&document).map_err(|_| bad_args("annotations"))?;
    host.out_write(&targets.annotations, &text)?;
    Ok(json!({
        "path": Targets::project(&targets.image),
        "annotations": Targets::project(&targets.annotations),
        "image": image,
        "format": targets.format.name(),
        "width": rendered.size.0,
        "height": rendered.size.1,
        "bytes": rendered.bytes,
    })
    .to_string())
}

/// What [`render_file`] made: the original's size, the edited image's, its length.
struct Rendered {
    source: (u32, u32),
    size: (u32, u32),
    bytes: usize,
}

/// `annotations` rendered onto the PNG `image` band by band into `targets.image`, written in
/// parts and committed at the end: a failure leaves no file there.
fn render_file(
    host: &mut dyn PluginHost,
    image: &str,
    annotations: &[Annotation],
    targets: &Targets,
    quality: u8,
) -> Result<Rendered, Failure> {
    let shared: Shared<'_> = RefCell::new(host);
    let mut source = PngRows::open(&shared, image)?;
    let (width, height) = source.size();
    let plan = band::plan(annotations, width, height, band::rows_per_band(width))?;
    let mut bands = Bands::new(&mut source, annotations, &plan);
    let size = bands.size();
    let mut out = OutFile::begin(&shared, &targets.image)?;
    match targets.format {
        Format::Png => encode::png(&mut bands, &mut out)?,
        Format::Jpeg => encode::jpeg(&mut bands, quality, &mut out)?,
    }
    let bytes = out.commit()?;
    Ok(Rendered {
        source: (width, height),
        size,
        bytes,
    })
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct LoadArgs {
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    offset: Option<usize>,
    /// The first chunk's `hash`: a later chunk comes from the cut version.
    #[serde(default)]
    hash: Option<String>,
    #[serde(default)]
    #[expect(dead_code, reason = "MCP adds it to every tool; an image has no view")]
    target: Option<Value>,
}

/// The version of an image: its fingerprint, length and size.
struct Version {
    hash: String,
    bytes: usize,
    width: u32,
    height: u32,
}

/// `plugin.screenshot-editor.panel.changed`: the file changed while it was read.
fn changed() -> Failure {
    Failure::coded(
        FailureKind::Other,
        &format!("plugin.{PLUGIN_ID}.panel.changed"),
    )
}

/// Reads `image` range by range (it may exceed one read): its fingerprint, its length, its size
/// from the header (held to the capture's limits) and the chunk at `offset` - one pass, so the
/// chunk is of the very bytes the fingerprint names (review of RB-099-24).
fn version(
    host: &mut dyn PluginHost,
    image: &str,
    offset: usize,
) -> Result<(Version, Vec<u8>), Failure> {
    let length = u32::try_from(MAX_IO_BYTES).unwrap_or(u32::MAX);
    let (first, size) = host.project_read_range(image, 0, length)?;
    let (width, height) = codec::dimensions(&first)?;
    let wanted = offset..offset.saturating_add(CHUNK_BYTES);
    let mut chunk = Vec::new();
    let mut hash = cache::FNV_START;
    let mut part = first;
    let mut at = 0usize;
    loop {
        hash = cache::fnv(hash, &part);
        let from = wanted.start.max(at);
        let to = wanted.end.min(at + part.len());
        if from < to {
            chunk.extend_from_slice(&part[from - at..to - at]);
        }
        at += part.len();
        if at as u64 >= size {
            break;
        }
        let (next, now) = host.project_read_range(image, at as u64, length)?;
        if next.is_empty() || now != size {
            return Err(changed());
        }
        part = next;
    }
    let version = Version {
        hash: format!("{hash:016x}"),
        bytes: usize::try_from(size).unwrap_or(usize::MAX),
        width,
        height,
    };
    Ok((version, chunk))
}

/// `plugin.screenshot-editor.errors.preview_unavailable`: the scaled view of a large image could
/// not be cut for the panel (the cache is full or not writable), naming its folder.
fn preview_unavailable() -> Failure {
    Failure::coded(
        FailureKind::Other,
        &format!("plugin.{PLUGIN_ID}.errors.preview_unavailable"),
    )
    .with_param(
        "folder",
        format!("{}{}/", crate::paths::OWN_ROOT, cache::CHUNKS_DIR),
    )
}

/// The annotations saved for `image` beside its edit, if they read; a file that does not read
/// is logged and left alone.
fn saved(host: &mut dyn PluginHost, image: &str) -> Option<Document> {
    let path = Targets::project(&paths::mirror(image, Format::Png).annotations);
    let bytes = host.project_read(&path).ok()?;
    let document = serde_json::from_slice::<Document>(&bytes)
        .ok()
        .and_then(|mut document| {
            validate(&mut document.annotations).ok()?;
            Some(document)
        });
    if document.is_none() {
        host.log(
            "warn",
            "the saved annotations do not read; starting without them",
        );
    }
    document
}

/// One chunk of `image` (the last screenshot without one) from `offset`; the first also names
/// the size and the saved annotations.
pub fn load(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let LoadArgs {
        image,
        offset,
        hash,
        ..
    } = parse(args)?;
    let image = match image {
        Some(image) => image,
        None => files::last(host)?.ok_or_else(|| {
            Failure::coded(
                FailureKind::NotFound,
                &format!("plugin.{PLUGIN_ID}.errors.no_screenshot"),
            )
        })?,
    };
    let image = paths::source(&image)?;
    let offset = offset.unwrap_or(0);
    // A call naming the first answer's `hash` asks for image data only (RB-080-63: from offset 0
    // too, when the first answer had no room for its chunk).
    if let Some(hash) = hash.as_deref() {
        match cache::chunk(host, hash, offset) {
            Cached::Data { data, bytes } => {
                let end = (offset + CHUNK_BYTES).min(bytes);
                return Ok(json!({
                    "image": image,
                    "bytes": bytes,
                    "offset": offset,
                    "next": if end < bytes { json!(end) } else { Value::Null },
                    "hash": hash,
                    "data": data,
                })
                .to_string());
            }
            Cached::PastEnd => return Err(bad_args("offset")),
            Cached::Missing => {}
        }
    }
    let (version, chunk) = version(host, &image, offset)?;
    let scale = preview::scale(version.width, version.height, version.bytes);
    // Offset 0 is a first read: it cuts the image and names its size, also when it names a
    // `hash` the cache does not hold (review of RB-080-63). Only the call without a hash carries
    // the annotations; the one with a hash fetches the data the first answer had no room for, so
    // its data never stays out.
    let fetch = hash.is_some();
    let (bytes, data) = if scale > 1 {
        // The preview is made and cut at the first call; a later chunk comes from the cache.
        if offset != 0 {
            return Err(preview_unavailable());
        }
        match cache::chunk(host, &version.hash, 0) {
            // Cut by an earlier first call: the image is not decoded again.
            Cached::Data { data, bytes } => (bytes, data),
            Cached::PastEnd | Cached::Missing => {
                let preview = preview::encode(host, &image, scale)?;
                // Made from other bytes than the fingerprint names: replaced in between.
                if preview.hash.as_deref() != Some(version.hash.as_str()) {
                    return Err(changed());
                }
                let shown = preview.png;
                let mut read = |_: &mut dyn PluginHost,
                                at: usize,
                                length: usize|
                 -> Result<Vec<u8>, Failure> {
                    Ok(shown[at..at + length].to_vec())
                };
                let source = cache::Source::Preview;
                if !cache::prepare(host, &version.hash, shown.len(), &mut read, source) {
                    return Err(preview_unavailable());
                }
                let end = CHUNK_BYTES.min(shown.len());
                (shown.len(), STANDARD.encode(&shown[..end]))
            }
        }
    } else {
        if offset > version.bytes {
            return Err(bad_args("offset"));
        }
        if offset == 0 {
            let mut read =
                |host: &mut dyn PluginHost, at: usize, length: usize| -> Result<Vec<u8>, Failure> {
                    let length = u32::try_from(length).unwrap_or(u32::MAX);
                    host.project_read_range(&image, at as u64, length)
                        .map(|(part, _)| part)
                };
            let source = cache::Source::Original;
            cache::prepare(host, &version.hash, version.bytes, &mut read, source);
        }
        (version.bytes, STANDARD.encode(&chunk))
    };
    let end = (offset + CHUNK_BYTES).min(bytes);
    let mut answer = json!({
        "image": image,
        "bytes": bytes,
        "offset": offset,
        "next": if end < bytes { json!(end) } else { Value::Null },
        "hash": version.hash,
        "data": data,
    });
    if offset == 0 {
        answer["width"] = json!(version.width);
        answer["height"] = json!(version.height);
        answer["scale"] = json!(scale);
    }
    if offset == 0 && !fetch {
        answer["annotations"] =
            saved(host, &image).map_or(Value::Null, |document| json!(document.annotations));
        fit_first(host, &mut answer);
    }
    Ok(answer.to_string())
}

/// Holds the first answer to [`ANSWER_BYTES`] (RB-080-63): a whole chunk and the saved
/// annotations together may exceed a panel message. Then the chunk stays out (`data` empty,
/// `next` 0: the next call fetches it with the `hash`); annotations that do not fit even alone
/// stay out instead (`annotations_too_large`), and the image opens without them - with the
/// chunk when it fits then, else without (review of RB-080-63: a long path).
fn fit_first(host: &mut dyn PluginHost, answer: &mut Value) {
    if answer.to_string().len() <= ANSWER_BYTES {
        return;
    }
    let data = std::mem::replace(&mut answer["data"], json!(""));
    let next = std::mem::replace(&mut answer["next"], json!(0));
    if answer.to_string().len() <= ANSWER_BYTES {
        return;
    }
    answer["annotations"] = Value::Null;
    answer["annotations_too_large"] = json!(true);
    host.log(
        "warn",
        "the saved annotations exceed a panel message; the image opens without them",
    );
    let data = std::mem::replace(&mut answer["data"], data);
    let next = std::mem::replace(&mut answer["next"], next);
    if answer.to_string().len() > ANSWER_BYTES {
        answer["data"] = data;
        answer["next"] = next;
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    #[serde(default)]
    #[expect(dead_code, reason = "MCP adds it to every tool; an image has no view")]
    target: Option<Value>,
}

/// The images the editor can open, the newest screenshots first.
pub fn list(host: &mut dyn PluginHost, args: &str) -> Result<String, Failure> {
    let ListArgs { .. } = parse(args)?;
    let (images, truncated) = files::list(host)?;
    Ok(json!({ "images": images, "truncated": truncated }).to_string())
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tools_large_tests.rs"]
mod large_tests;
