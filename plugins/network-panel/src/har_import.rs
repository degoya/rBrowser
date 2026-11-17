//! HAR import (`har_import`): a HAR file below `.rbrowser/` replayed as mocks - one rule per
//! method and URL (the first answer with a body wins, else the first), matching that exact URL, answering its status,
//! headers and body. The rules go to the plugin's own `mocks/har-<name>.json`, so they are
//! project content like every mock: they apply only in a trusted workspace.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::{Value, json};

use crate::mocks::{self, Body, Kind, MockRule};

/// The most rules one import makes.
pub const MAX_RULES: usize = 200;
/// The most body bytes (base64 or text) one import keeps.
pub const MAX_BODIES: usize = 6 * 1024 * 1024;

/// Response headers a replay must not repeat: the body is served whole and decoded.
const DROPPED: &[&str] = &[
    "connection",
    "content-encoding",
    "content-length",
    "keep-alive",
    "transfer-encoding",
];

/// `text` as a glob matching exactly itself.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '*' | '?' | '\\') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// The rules of a HAR's text, and how many entries were skipped.
///
/// # Errors
///
/// The text is no HAR (`log.entries`).
pub fn rules_of(text: &[u8]) -> Result<(Vec<MockRule>, usize), String> {
    let har: Value = serde_json::from_slice(text).map_err(|_| "no JSON".to_owned())?;
    let entries = har["log"]["entries"]
        .as_array()
        .ok_or_else(|| "no log.entries".to_owned())?;
    let mut rules = Vec::new();
    let mut seen: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut skipped = 0;
    let mut bytes = 0;
    for entry in entries {
        let request = &entry["request"];
        let response = &entry["response"];
        let (Some(method), Some(url)) = (request["method"].as_str(), request["url"].as_str())
        else {
            skipped += 1;
            continue;
        };
        let status = response["status"]
            .as_u64()
            .and_then(|status| u16::try_from(status).ok())
            .filter(|status| (100..=599).contains(status));
        let web = url.starts_with("http://") || url.starts_with("https://");
        let (Some(status), true) = (status, web) else {
            skipped += 1;
            continue;
        };
        let key = (method.to_ascii_uppercase(), url.to_owned());
        let earlier = seen.get(&key).copied();
        let content = &response["content"];
        let body = match content["text"].as_str() {
            None => Body::None,
            Some(text) if content["encoding"] == "base64" => Body::Base64(text.to_owned()),
            Some(text) => Body::Text(text.to_owned()),
        };
        let size = match &body {
            Body::Text(text) | Body::Base64(text) => text.len(),
            Body::None | Body::File(_) => 0,
        };
        if let Some(at) = earlier {
            // A later answer of the same request only fills in a body the first had not.
            let filled = matches!(
                &rules[at],
                MockRule {
                    kind: Kind::Mock {
                        body: Body::None,
                        ..
                    },
                    ..
                }
            ) && size > 0
                && bytes + size <= MAX_BODIES;
            if filled && let Kind::Mock { body: first, .. } = &mut rules[at].kind {
                *first = body;
                bytes += size;
            }
            continue;
        }
        if rules.len() >= MAX_RULES || bytes + size > MAX_BODIES {
            skipped += 1;
            continue;
        }
        bytes += size;
        let mut headers: Vec<(String, String)> = response["headers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|header| {
                Some((
                    header["name"].as_str()?.to_owned(),
                    header["value"].as_str()?.to_owned(),
                ))
            })
            .filter(|(name, _)| !DROPPED.contains(&name.to_ascii_lowercase().as_str()))
            .collect();
        if let Some(mime) = content["mimeType"]
            .as_str()
            .filter(|mime| *mime != "x-unknown")
            && !headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        {
            headers.push(("content-type".to_owned(), mime.to_owned()));
        }
        seen.insert(key, rules.len());
        rules.push(MockRule {
            id: format!("e{}", rules.len() + 1),
            enabled: true,
            url_pattern: escape(url),
            regex: false,
            method: Some(method.to_ascii_uppercase()),
            resource_type: None,
            kind: Kind::Mock {
                status,
                headers,
                body,
                delay_ms: 0,
            },
        });
    }
    Ok((rules, skipped))
}

/// Imports the HAR at `path` (below `.rbrowser/`, ending in `.har`) into
/// `mocks/har-<name>.json` and answers where, with the counts.
///
/// # Errors
///
/// `plugin.network-panel.errors.har_path` for another path, `...har_read` when it cannot be read,
/// `...har_invalid` when it is no HAR, or the write's failure.
pub fn import(host: &mut dyn PluginHost, path: &str) -> Result<Value, Failure> {
    let name = path
        .strip_prefix(".rbrowser/")
        .filter(|rest| mocks::relative_path(rest))
        .and_then(|_| path.rsplit('/').next())
        .and_then(|file| file.strip_suffix(".har"))
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| crate::coded(FailureKind::BadData, "errors.har_path"))?;
    let text = host
        .project_read(path)
        .map_err(|_| crate::coded(FailureKind::NotFound, "errors.har_read"))?;
    let (rules, skipped) = rules_of(&text).map_err(|reason| {
        crate::coded(FailureKind::BadData, "errors.har_invalid").with_param("reason", reason)
    })?;
    let file = format!("har-{}.json", mocks::prefix_of(name));
    host.out_write(
        &format!("{}/{file}", mocks::OWN_DIR),
        mocks::file_json(&rules).to_string().as_bytes(),
    )?;
    Ok(json!({
        "path": format!("{}/{file}", mocks::OWN_PATH),
        "rules": rules.len(),
        "skipped": skipped,
    }))
}

#[cfg(test)]
#[path = "har_import_tests.rs"]
mod tests;
