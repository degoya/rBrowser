//! The bytes of a web font for the panel's preview (RB-080-83). The panel has no network, so it
//! asks the tool `preview` for the file a view loaded, part by part: the view reads it again with
//! `fetch` (from the browser's cache, with the page's origin - a web font that loaded is readable
//! there, as fonts load with CORS) and answers one part as base64. A part stays under the panels
//! window's 64 KiB per message (RB-080-63); the panel puts the parts together and loads the font
//! with `FontFace`. Only a file the page itself loaded is read - one in its resource timing or in
//! one of its readable `@font-face` rules -, never any other address, and none larger than the
//! setting `previewMaxKb`. Every refusal has its reason: `too_large`, `not_listed`, `unreadable`.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde_json::{Value, json};

/// The bytes of one part: as base64 with the answer around it, under 64 KiB.
pub const CHUNK_BYTES: usize = 40 * 1024;
/// The longest font address taken.
pub const MAX_SRC_CHARS: usize = 2048;
/// `previewMaxKb`: the default, and the schema's bounds.
pub const DEFAULT_MAX_KB: u64 = 1024;
pub const MIN_KB: u64 = 16;
pub const MAX_KB: u64 = 4096;

/// The reasons the page gives for no bytes.
const REASONS: [&str; 3] = ["too_large", "not_listed", "unreadable"];

/// The page part: reads `request.src` again if the page loaded it, and answers one part.
const SCRIPT: &str = r#"(async (request) => {
  const listed = new Set(performance.getEntriesByType('resource').map((entry) => entry.name));
  for (const sheet of Array.from(document.styleSheets)) {
    let rules;
    try { rules = sheet.cssRules; } catch { continue; }
    for (const rule of Array.from(rules)) {
      if (!(rule instanceof CSSFontFaceRule)) continue;
      for (const match of rule.style.getPropertyValue('src').matchAll(/url\(\s*["']?([^"')]+)["']?\s*\)/g)) {
        try { listed.add(new URL(match[1], sheet.href || document.baseURI).href); } catch { /* not an address */ }
      }
    }
  }
  if (!listed.has(request.src)) return { error: 'not_listed' };
  let response;
  try { response = await fetch(request.src, { cache: 'force-cache', credentials: 'same-origin' }); } catch { return { error: 'unreadable' }; }
  if (!response.ok) return { error: 'unreadable' };
  const declared = Number(response.headers.get('content-length'));
  if (declared > request.max) return { error: 'too_large', total: declared };
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length > request.max) return { error: 'too_large', total: bytes.length };
  const part = bytes.subarray(request.offset, request.offset + request.length);
  let binary = '';
  for (let at = 0; at < part.length; at += 8192) binary += String.fromCharCode.apply(null, Array.from(part.subarray(at, at + 8192)));
  return { total: bytes.length, offset: request.offset, data: btoa(binary) };
})"#;

fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

fn malformed() -> Failure {
    Failure::coded(FailureKind::BadData, codes::HOST_FAILED).with_param("reason", "answer")
}

/// The setting `previewMaxKb`, held to the schema's bounds.
pub fn max_kb(host: &mut dyn PluginHost) -> u64 {
    host.setting("previewMaxKb")
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.as_u64())
        .map_or(DEFAULT_MAX_KB, |kb| kb.clamp(MIN_KB, MAX_KB))
}

/// An `http:` or `https:` address without control characters, at most [`MAX_SRC_CHARS`] long.
fn src(args: &Value) -> Result<&str, Failure> {
    let text = args["src"].as_str().ok_or_else(|| invalid("src"))?;
    let web = text.starts_with("https://") || text.starts_with("http://");
    if !web || text.chars().count() > MAX_SRC_CHARS || text.chars().any(char::is_control) {
        return Err(invalid("src"));
    }
    Ok(text)
}

/// The expression that reads one part of `src` in a view.
#[must_use]
pub fn script(src: &str, offset: u64, max_bytes: u64) -> String {
    let request = json!({ "src": src, "offset": offset, "length": CHUNK_BYTES, "max": max_bytes });
    // The request is JSON text: data, never code, inside the expression.
    format!("{SCRIPT}({request})")
}

/// Whether `data` is base64 of at most one part.
fn is_part(data: &str) -> bool {
    data.len() <= CHUNK_BYTES.div_ceil(3) * 4
        && data
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

/// `preview`: one part of the font file `src` as view `view` loaded it, from `offset` (0 when
/// left out): `{src, offset, total, data, done}`, or `{src, error, maxKb}` with the reason there
/// is none.
///
/// # Errors
///
/// `plugins.error.invalid` for malformed arguments, the host's failure to evaluate, and
/// `plugins.error.host_failed` for an answer the page part would never give.
pub fn bytes(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let src = src(args)?.to_owned();
    let view = args["view"]
        .as_u64()
        .and_then(|id| ViewId::try_from(id).ok())
        .ok_or_else(|| invalid("view"))?;
    let max_kb = max_kb(host);
    let offset = match args.get("offset") {
        None | Some(Value::Null) => 0,
        Some(value) => value
            .as_u64()
            .filter(|offset| *offset < max_kb * 1024)
            .ok_or_else(|| invalid("offset"))?,
    };
    let answer = host.evaluate(view, &script(&src, offset, max_kb * 1024))?;
    let page: Value = serde_json::from_str(&answer).map_err(|_| malformed())?;
    if let Some(reason) = page["error"].as_str() {
        if !REASONS.contains(&reason) {
            return Err(malformed());
        }
        return Ok(json!({ "src": src, "error": reason, "maxKb": max_kb }));
    }
    let total = page["total"].as_u64().ok_or_else(malformed)?;
    let data = page["data"]
        .as_str()
        .filter(|data| is_part(data))
        .ok_or_else(malformed)?;
    if page["offset"].as_u64() != Some(offset) || total > max_kb * 1024 {
        return Err(malformed());
    }
    let padding = data.bytes().rev().take_while(|b| *b == b'=').count();
    let read = u64::try_from((data.len() / 4 * 3).saturating_sub(padding)).unwrap_or(u64::MAX);
    Ok(json!({
        "src": src,
        "offset": offset,
        "total": total,
        "data": data,
        "done": offset.saturating_add(read) >= total,
    }))
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
