//! Masking for everything that leaves the person's own window (RB-080-37): MCP answers, the
//! session files and log lines carry no cookie, token or password. A header of a sensitive name
//! keeps its name and gets `***`; a URL's query parameter of a sensitive name gets `***`. The
//! lists and the URL and header masking are `rb_util::mask`, shared with the plugins.

use serde_json::{Value, json};

pub use rb_util::mask::{
    MASK, SENSITIVE_HEADERS, SENSITIVE_PARAMS, mask_headers, mask_url, sensitive_header,
    sensitive_param,
};

/// Masks a held request's or a composed result's JSON in place: `url`, `headers` (as
/// `[[name, value]]` pairs) and `response.headers`.
pub fn mask_entry(entry: &mut Value) {
    if let Some(url) = entry["url"].as_str() {
        entry["url"] = json!(mask_url(url));
    }
    mask_pairs(&mut entry["headers"]);
    mask_pairs(&mut entry["response"]["headers"]);
}

/// Masks the sensitive values of a `[[name, value]]` list in place.
fn mask_pairs(headers: &mut Value) {
    if let Value::Array(list) = headers {
        for pair in list.iter_mut() {
            if let Some(name) = pair[0].as_str()
                && sensitive_header(name)
            {
                pair[1] = json!(MASK);
            }
        }
    }
}

#[cfg(test)]
#[path = "mask_tests.rs"]
mod tests;
