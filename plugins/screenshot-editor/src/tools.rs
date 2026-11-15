//! The plugin's tools - the panel's calls and MCP's `shot_annotate`, `shot_load`, `shot_list`:
//!
//! - [`annotate`] renders annotations onto an image and writes the result and the annotations
//!   below `.rbrowser/out/screenshot-editor/`, without any UI;
//! - [`load`] hands an image to the panel in chunks of [`CHUNK_BYTES`] (base64, so an answer
//!   stays below a panel message's 64 KiB), the first with its size and saved annotations, held
//!   to [`ANSWER_BYTES`] as a whole; the first cuts the image once ([`crate::cache`]), so a later
//!   chunk naming its `hash` is one small read;
//! - [`list`] names the images the editor can open.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::cache::{self, Cached};
use crate::model::{Annotation, DOCUMENT_VERSION, Document, validate};
use crate::paths::{self, Format, Targets};
use crate::{PLUGIN_ID, bad_args, codec, files, render, settings};

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
    let bytes = host.project_read(&image)?;
    let original = codec::decode(&bytes)?;
    let (width, height) = (original.width(), original.height());
    let rendered = render::render(original, &annotations)?;
    let encoded = match targets.format {
        Format::Png => codec::encode_png(&rendered)?,
        Format::Jpeg => codec::encode_jpeg(&rendered, settings.jpeg_quality)?,
    };
    if encoded.len() > rb_plugin_api::MAX_IO_BYTES {
        return Err(codec::output_too_large(
            encoded.len(),
            rb_plugin_api::MAX_IO_BYTES,
        ));
    }
    host.out_write(&targets.image, &encoded)?;
    let document = Document {
        version: DOCUMENT_VERSION,
        image: image.clone(),
        width,
        height,
        annotations,
    };
    let text = serde_json::to_vec_pretty(&document).map_err(|_| bad_args("annotations"))?;
    host.out_write(&targets.annotations, &text)?;
    Ok(json!({
        "path": Targets::project(&targets.image),
        "annotations": Targets::project(&targets.annotations),
        "image": image,
        "format": targets.format.name(),
        "width": rendered.width(),
        "height": rendered.height(),
        "bytes": encoded.len(),
    })
    .to_string())
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

/// FNV-1a, 64 bit: the panel checks every chunk came from the same file.
fn fingerprint(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
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
    let bytes = host.project_read(&image)?;
    if offset > bytes.len() {
        return Err(bad_args("offset"));
    }
    let end = (offset + CHUNK_BYTES).min(bytes.len());
    // Offset 0 is a first read: it cuts the image and names its size, also when it names a
    // `hash` the cache does not hold (review of RB-080-63). Only the call without a hash carries
    // the annotations; the one with a hash fetches the data the first answer had no room for, so
    // its data never stays out.
    let fetch = hash.is_some();
    let hash = fingerprint(&bytes);
    let mut answer = json!({
        "image": image,
        "bytes": bytes.len(),
        "offset": offset,
        "next": if end < bytes.len() { json!(end) } else { Value::Null },
        "hash": hash,
        "data": STANDARD.encode(&bytes[offset..end]),
    });
    if offset == 0 {
        cache::prepare(host, &bytes, &hash);
        let (width, height) = codec::dimensions(&bytes)?;
        answer["width"] = json!(width);
        answer["height"] = json!(height);
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
