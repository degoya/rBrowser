//! The page script's one request (`call-tool("@page")`, `page/pins.js`): a document of a view
//! started. The answer is the record of the storage values pinned for its origin and the view's
//! session - the page script sets them and keeps the record in the tab's sessionStorage, where
//! it finds them at the next document start before the page's own scripts - and the pinned
//! cookies of its host are set again over CDP (they stay in the session's jar, so the next
//! request carries them). The open panels are told to read again.

use rb_plugin_api::{Failure, PluginHost, ViewId};
use serde_json::{Value, json};

use crate::bad_args;
use crate::columns;
use crate::origin;
use crate::pins;
use crate::settings::Settings;

/// What `page/pins.js` sends when a document starts.
pub const LOADED: &str = "loaded";

/// `@page`: `args` is `{"view": <id>, "message": {"type": "loaded", "origin": <origin>}}`.
///
/// # Errors
///
/// A message that does not read.
pub fn request(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let view = args["view"]
        .as_u64()
        .and_then(|id| ViewId::try_from(id).ok())
        .ok_or_else(|| bad_args("view"))?;
    let message = &args["message"];
    if message["type"] != LOADED {
        return Err(bad_args("message"));
    }
    let post = json!([{ "target": "panel", "message": { "type": "changed", "view": view } }]);
    // The origin the document reports itself; a page without one (about:blank) gets nothing.
    let doc_origin = message["origin"]
        .as_str()
        .filter(|text| origin::is_origin(text));
    let column = columns::columns(host)
        .into_iter()
        .find(|column| column.id == view);
    let (Some(doc_origin), Some(column)) = (doc_origin, column) else {
        return Ok(json!({ "record": null, "$post": post }));
    };
    if !Settings::read(host).pins_enabled {
        return Ok(json!({ "record": null, "$post": post }));
    }
    let pins = pins::load(host);
    let failed = pins::apply_cookies(host, &column, &pins, doc_origin);
    if !failed.is_empty() {
        host.log(
            "warn",
            &format!("view {view}: {} pinned cookies not set", failed.len()),
        );
    }
    let record = pins::record(&pins, &column.session, doc_origin);
    let record = if pins::record_is_empty(&record) {
        Value::Null
    } else {
        json!({ "origin": doc_origin, "local": record["local"], "session": record["session"] })
    };
    Ok(json!({ "record": record, "$post": post }))
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
