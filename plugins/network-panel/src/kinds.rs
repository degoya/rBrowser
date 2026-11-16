//! What a rule of a mock file does ([`Kind`]), in the file form and in the host's: `mock`,
//! `block`, `rewrite` (URL, request and response headers, query parameters, a body replacement;
//! RB-080-37), `mapLocal` (a folder or file; RB-080-37) and `mapRemote` (host, port, path
//! prefix; RB-080-37). The host validates again and bounds the map-local paths; this module only
//! translates.
//!
//! ```json
//! { "id": "csp-off", "match": { "urlPattern": "*" },
//!   "rewrite": { "responseHeaders": [{ "op": "remove", "name": "content-security-policy" }],
//!                "query": [{ "op": "set", "name": "debug", "value": "1" }],
//!                "body": { "regex": "Hello (\\w+)", "replacement": "Bye $1" } } }
//! { "id": "local", "match": { "urlPattern": "*/config.json" }, "mapLocal": { "folder": "fixtures/api" } }
//! { "id": "remote", "match": { "urlPattern": "https://api.example.com/*" },
//!   "mapRemote": { "from": { "host": "api.example.com" }, "to": { "host": "localhost", "port": 3001 } } }
//! ```

use serde_json::{Map, Value, json};

/// A mock's body before it is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    None,
    Text(String),
    Base64(String),
    /// Relative to the mocks folder.
    File(String),
}

/// One change of a header list or a query (`op`: `set`, `add`, `remove`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Op {
    pub op: String,
    pub name: String,
    pub value: Option<String>,
}

/// What a rule does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Mock {
        status: u16,
        headers: Vec<(String, String)>,
        body: Body,
        delay_ms: u64,
    },
    Block,
    Rewrite {
        url: Option<String>,
        /// `headers` of the file: set operations.
        headers: Vec<(String, String)>,
        request_headers: Vec<Op>,
        response_headers: Vec<Op>,
        query: Vec<Op>,
        body: Option<(String, String)>,
    },
    MapLocal {
        folder: Option<String>,
        file: Option<String>,
        mime: Option<String>,
    },
    MapRemote {
        from: Value,
        to: Value,
    },
}

impl Kind {
    /// `mock`, `block`, `rewrite`, `mapLocal` or `mapRemote`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Mock { .. } => "mock",
            Self::Block => "block",
            Self::Rewrite { .. } => "rewrite",
            Self::MapLocal { .. } => "mapLocal",
            Self::MapRemote { .. } => "mapRemote",
        }
    }
}

/// The names of the kinds a rule may carry, one of them exactly.
pub const KEYS: &[&str] = &["mock", "block", "rewrite", "mapLocal", "mapRemote"];

fn headers(raw: &Value) -> Result<Vec<(String, String)>, String> {
    match raw {
        Value::Null => Ok(Vec::new()),
        Value::Object(map) => map
            .iter()
            .map(|(name, value)| match value {
                Value::String(text) => Ok((name.clone(), text.clone())),
                Value::Number(n) => Ok((name.clone(), n.to_string())),
                _ => Err(format!("header {name} is no text")),
            })
            .collect(),
        _ => Err("headers is no object".to_owned()),
    }
}

fn ops(raw: &Value, what: &str) -> Result<Vec<Op>, String> {
    match raw {
        Value::Null => Ok(Vec::new()),
        Value::Array(list) => list
            .iter()
            .map(|entry| {
                let op = entry["op"]
                    .as_str()
                    .filter(|op| ["set", "add", "remove"].contains(op))
                    .ok_or_else(|| format!("{what}: op is set, add or remove"))?;
                let name = entry["name"]
                    .as_str()
                    .filter(|name| !name.is_empty())
                    .ok_or_else(|| format!("{what}: an operation has no name"))?;
                let value = match &entry["value"] {
                    Value::Null => None,
                    Value::String(text) => Some(text.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    _ => return Err(format!("{what}: {name} has no text value")),
                };
                if op != "remove" && value.is_none() {
                    return Err(format!("{what}: {name} needs a value"));
                }
                Ok(Op {
                    op: op.to_owned(),
                    name: name.to_owned(),
                    value,
                })
            })
            .collect(),
        _ => Err(format!("{what} is no list")),
    }
}

fn text(raw: &Value, what: &str) -> Result<Option<String>, String> {
    match raw {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text.clone())),
        _ => Err(format!("{what} is no text")),
    }
}

fn relative(path: &str) -> bool {
    super::mocks::relative_path(path)
}

fn body(mock: &Value) -> Result<Body, String> {
    let given = ["body", "bodyBase64", "bodyFile"]
        .iter()
        .filter(|key| !mock[**key].is_null())
        .count();
    if given > 1 {
        return Err("more than one of body, bodyBase64, bodyFile".to_owned());
    }
    Ok(
        match (&mock["body"], &mock["bodyBase64"], &mock["bodyFile"]) {
            (Value::String(text), _, _) => Body::Text(text.clone()),
            (Value::Null, Value::Null, Value::Null) => Body::None,
            (Value::Null, Value::String(data), _) => Body::Base64(data.clone()),
            (Value::Null, Value::Null, Value::String(file)) if relative(file) => {
                Body::File(file.clone())
            }
            (Value::Null, Value::Null, _) => {
                return Err("bodyFile is no plain relative path".to_owned());
            }
            (Value::Null, _, _) => return Err("bodyBase64 is no text".to_owned()),
            (json, _, _) => Body::Text(json.to_string()),
        },
    )
}

fn rewrite(rewrite: &Value) -> Result<Kind, String> {
    let body = match &rewrite["body"] {
        Value::Null => None,
        Value::Object(map) => Some((
            map.get("regex")
                .and_then(Value::as_str)
                .filter(|regex| !regex.is_empty())
                .ok_or("rewrite.body.regex is missing")?
                .to_owned(),
            map.get("replacement")
                .and_then(Value::as_str)
                .ok_or("rewrite.body.replacement is missing")?
                .to_owned(),
        )),
        _ => return Err("rewrite.body is no object".to_owned()),
    };
    Ok(Kind::Rewrite {
        url: text(&rewrite["url"], "rewrite.url")?,
        headers: headers(&rewrite["headers"])?,
        request_headers: ops(&rewrite["requestHeaders"], "rewrite.requestHeaders")?,
        response_headers: ops(&rewrite["responseHeaders"], "rewrite.responseHeaders")?,
        query: ops(&rewrite["query"], "rewrite.query")?,
        body,
    })
}

fn map_local(local: &Value) -> Result<Kind, String> {
    let folder = text(&local["folder"], "mapLocal.folder")?;
    let file = text(&local["file"], "mapLocal.file")?;
    if folder.is_some() == file.is_some() {
        return Err("mapLocal needs one of folder, file".to_owned());
    }
    Ok(Kind::MapLocal {
        folder,
        file,
        mime: text(&local["mime"], "mapLocal.mime")?,
    })
}

fn endpoint(raw: &Value, what: &str) -> Result<Value, String> {
    let Value::Object(map) = raw else {
        return Err(format!("{what} is no object"));
    };
    map.get("host")
        .and_then(Value::as_str)
        .filter(|host| !host.is_empty())
        .ok_or_else(|| format!("{what}.host is missing"))?;
    let mut out = Map::new();
    for key in ["scheme", "host", "port", "path"] {
        if let Some(value) = map.get(key) {
            out.insert(key.to_owned(), value.clone());
        }
    }
    Ok(Value::Object(out))
}

/// The kind of a rule as it is written: exactly one of [`KEYS`].
///
/// # Errors
///
/// What does not read, as one line.
pub fn parse(raw: &Value) -> Result<Kind, String> {
    let given = KEYS
        .iter()
        .filter(|key| !raw[**key].is_null() && raw[**key] != Value::Bool(false))
        .count();
    if given != 1 {
        return Err("needs exactly one of mock, block, rewrite, mapLocal, mapRemote".to_owned());
    }
    if raw["block"] == Value::Bool(true) {
        return Ok(Kind::Block);
    }
    if let Value::Object(_) = &raw["rewrite"] {
        return rewrite(&raw["rewrite"]);
    }
    if let Value::Object(_) = &raw["mapLocal"] {
        return map_local(&raw["mapLocal"]);
    }
    if let Value::Object(_) = &raw["mapRemote"] {
        return Ok(Kind::MapRemote {
            from: endpoint(&raw["mapRemote"]["from"], "mapRemote.from")?,
            to: endpoint(&raw["mapRemote"]["to"], "mapRemote.to")?,
        });
    }
    let mock = &raw["mock"];
    if !mock.is_object() {
        return Err("mock is no object".to_owned());
    }
    let status = match &mock["status"] {
        Value::Null => 200,
        value => value
            .as_u64()
            .and_then(|status| u16::try_from(status).ok())
            .filter(|status| (100..=599).contains(status))
            .ok_or("mock.status is no HTTP status")?,
    };
    let delay_ms = match &mock["delayMs"] {
        Value::Null => 0,
        value => value
            .as_u64()
            .filter(|delay| *delay <= 60_000)
            .ok_or("mock.delayMs is no delay up to 60000")?,
    };
    Ok(Kind::Mock {
        status,
        headers: headers(&mock["headers"])?,
        body: body(mock)?,
        delay_ms,
    })
}

fn header_list(headers: &[(String, String)]) -> Value {
    Value::Array(
        headers
            .iter()
            .map(|(name, value)| json!([name, value]))
            .collect(),
    )
}

fn header_map(headers: &[(String, String)]) -> Value {
    Value::Object(
        headers
            .iter()
            .map(|(name, value)| (name.clone(), json!(value)))
            .collect::<Map<String, Value>>(),
    )
}

fn op_list(ops: &[Op]) -> Value {
    Value::Array(
        ops.iter()
            .map(|op| {
                let mut entry = json!({ "op": op.op, "name": op.name });
                if let Some(value) = &op.value {
                    entry["value"] = json!(value);
                }
                entry
            })
            .collect(),
    )
}

/// `kind` as the host's `action`; a mock's body is `body_base64`, read by the caller.
#[must_use]
pub fn host_action(kind: &Kind, body_base64: &str) -> Value {
    match kind {
        Kind::Block => json!({ "type": "block" }),
        Kind::Rewrite {
            url,
            headers,
            request_headers,
            response_headers,
            query,
            body,
        } => {
            let mut action = json!({ "type": "rewrite", "headers": header_list(headers) });
            if let Some(url) = url {
                action["url"] = json!(url);
            }
            if !request_headers.is_empty() {
                action["requestHeaders"] = op_list(request_headers);
            }
            if !response_headers.is_empty() {
                action["responseHeaders"] = op_list(response_headers);
            }
            if !query.is_empty() {
                action["query"] = op_list(query);
            }
            if let Some((regex, replacement)) = body {
                action["body"] = json!({ "regex": regex, "replacement": replacement });
            }
            action
        }
        Kind::Mock {
            status,
            headers,
            delay_ms,
            ..
        } => json!({ "type": "mock", "status": status, "headers": header_list(headers),
                     "bodyBase64": body_base64, "delayMs": delay_ms }),
        Kind::MapLocal { folder, file, mime } => {
            let mut action = json!({ "type": "mapLocal" });
            for (key, value) in [("folder", folder), ("file", file), ("mime", mime)] {
                if let Some(value) = value {
                    action[key] = json!(value);
                }
            }
            action
        }
        Kind::MapRemote { from, to } => json!({ "type": "mapRemote", "from": from, "to": to }),
    }
}

/// Writes `kind` into `out` in the file form, as [`parse`] reads it back.
pub fn write_json(kind: &Kind, out: &mut Value) {
    match kind {
        Kind::Block => out["block"] = json!(true),
        Kind::Rewrite {
            url,
            headers,
            request_headers,
            response_headers,
            query,
            body,
        } => {
            let mut rewrite = json!({ "headers": header_map(headers) });
            if let Some(url) = url {
                rewrite["url"] = json!(url);
            }
            for (key, ops) in [
                ("requestHeaders", request_headers),
                ("responseHeaders", response_headers),
                ("query", query),
            ] {
                if !ops.is_empty() {
                    rewrite[key] = op_list(ops);
                }
            }
            if let Some((regex, replacement)) = body {
                rewrite["body"] = json!({ "regex": regex, "replacement": replacement });
            }
            out["rewrite"] = rewrite;
        }
        Kind::Mock {
            status,
            headers,
            body,
            delay_ms,
        } => {
            let mut mock = json!({ "status": status, "headers": header_map(headers),
                                   "delayMs": delay_ms });
            match body {
                Body::None => {}
                Body::Text(text) => mock["body"] = json!(text),
                Body::Base64(data) => mock["bodyBase64"] = json!(data),
                Body::File(file) => mock["bodyFile"] = json!(file),
            }
            out["mock"] = mock;
        }
        Kind::MapLocal { folder, file, mime } => {
            let mut local = json!({});
            for (key, value) in [("folder", folder), ("file", file), ("mime", mime)] {
                if let Some(value) = value {
                    local[key] = json!(value);
                }
            }
            out["mapLocal"] = local;
        }
        Kind::MapRemote { from, to } => out["mapRemote"] = json!({ "from": from, "to": to }),
    }
}

#[cfg(test)]
#[path = "kinds_tests.rs"]
mod tests;
