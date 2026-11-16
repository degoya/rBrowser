//! The MCP tools: `storage_get` - every targeted view's cookies, localStorage and
//! sessionStorage as JSON with the differences between the views - and `storage_set` - one
//! value written or removed, per view success or error. Values are user data: `storage_get`
//! answers them, and `storage_set` writes, only while a person allows it in the protected
//! setting `storage-panel.mcpValues`; without it `storage_get` answers names, attributes and
//! lengths and which views agree, never a value.

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::{Map, Value, json};

use crate::columns::{self, Target};
use crate::matrix::{self, Values};
use crate::read::{self, Kind, ViewData};
use crate::settings::Settings;
use crate::{bad_args, coded, write};

/// The longest value `storage_get` answers, in characters (`truncated` beyond).
pub const MCP_VALUE_CHARS: usize = 16 * 1024;

/// The filters of a read: `kinds`, `origin` (part of the origin or cookie domain) and `search`
/// (part of the name), both case-insensitive.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub kinds: Vec<Kind>,
    pub origin: Option<String>,
    pub search: Option<String>,
}

fn optional_text(args: &Value, key: &str, max: usize) -> Result<Option<String>, Failure> {
    match &args[key] {
        Value::Null => Ok(None),
        Value::String(text) if text.chars().count() <= max => {
            Ok(Some(text.trim().to_lowercase()).filter(|t| !t.is_empty()))
        }
        _ => Err(bad_args(key)),
    }
}

impl Filter {
    /// The filter of a call's arguments; `kinds` from `kinds` (a list) or `kind` (one).
    ///
    /// # Errors
    ///
    /// A field that does not read.
    pub fn parse(args: &Value) -> Result<Self, Failure> {
        let kinds = match (&args["kinds"], &args["kind"]) {
            (Value::Array(names), _) => names
                .iter()
                .map(|name| name.as_str().and_then(Kind::parse))
                .collect::<Option<Vec<Kind>>>()
                .ok_or_else(|| bad_args("kinds"))?,
            (Value::Null, Value::String(name)) => {
                vec![Kind::parse(name).ok_or_else(|| bad_args("kind"))?]
            }
            (Value::Null, Value::Null) => Kind::ALL.to_vec(),
            _ => return Err(bad_args("kinds")),
        };
        Ok(Self {
            kinds,
            origin: optional_text(args, "origin", 300)?,
            search: optional_text(args, "search", 200)?,
        })
    }

    /// Whether an entry of `scope` named `name` passes.
    #[must_use]
    pub fn passes(&self, scope: &str, name: &str) -> bool {
        self.origin
            .as_ref()
            .is_none_or(|part| scope.to_lowercase().contains(part))
            && self
                .search
                .as_ref()
                .is_none_or(|part| name.to_lowercase().contains(part))
    }
}

/// `target` of a call: `"all"`, a view id, or none (every view).
///
/// # Errors
///
/// Anything else.
pub fn target(args: &Value) -> Result<Target, Failure> {
    Target::parse(&args["target"]).ok_or_else(|| bad_args("target"))
}

fn entry(value: &str, values: Values) -> Map<String, Value> {
    matrix::value_json(value, values)
}

fn view_json(data: &ViewData, filter: &Filter, values: Values) -> Value {
    let mut out = data.column.to_json(data.status);
    if let Some(error) = &data.error {
        out["error"] = json!(error);
    }
    if data.status != "ok" {
        return out;
    }
    out["documentOrigin"] = json!(data.origin);
    if let Some(reason) = &data.storage_error {
        out["storageError"] = json!(reason);
    }
    for kind in &filter.kinds {
        let list: Vec<Value> = match kind {
            Kind::Cookies => data
                .cookies
                .iter()
                .filter(|cookie| filter.passes(&cookie.domain, &cookie.name))
                .map(|cookie| {
                    let mut item = entry(&cookie.value, values);
                    item.insert("name".to_owned(), json!(cookie.name));
                    if let Value::Object(attributes) = cookie.attributes() {
                        item.extend(attributes);
                    }
                    Value::Object(item)
                })
                .collect(),
            Kind::Local | Kind::Session => {
                let scope = data.origin.clone().unwrap_or_default();
                data.storage(*kind)
                    .iter()
                    .filter(|(key, _)| filter.passes(&scope, key))
                    .map(|(key, value)| {
                        let mut item = entry(value, values);
                        item.insert("key".to_owned(), json!(key));
                        Value::Object(item)
                    })
                    .collect()
            }
        };
        out[kind.name()] = json!(list);
    }
    if data.more > 0 {
        out["notRead"] = json!(data.more);
    }
    out
}

/// `storage_get`: per targeted view its cookies and storage, and the entries that differ
/// between the views (`groups` of views that agree, the views it is `missing` in).
///
/// # Errors
///
/// Arguments that do not read.
pub fn get(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let filter = Filter::parse(args)?;
    let target = target(args)?;
    let columns = columns::columns(host);
    let data = read::read_all(
        host,
        &columns,
        target,
        &filter.kinds,
        settings.show_http_only,
    );
    let values = if settings.mcp_values {
        Values::Shown(MCP_VALUE_CHARS)
    } else {
        Values::Hidden
    };
    let views: Vec<Value> = data
        .iter()
        .map(|view| view_json(view, &filter, values))
        .collect();
    let differences: Vec<Value> = filter
        .kinds
        .iter()
        .flat_map(|kind| matrix::rows(*kind, &data))
        .filter(|row| row.state() != matrix::State::Same && filter.passes(&row.scope, &row.name))
        .map(|row| matrix::difference_json(&row))
        .collect();
    Ok(json!({
        "values": if settings.mcp_values { "shown" } else { "hidden" },
        "views": views,
        "differences": differences,
    }))
}

/// `storage_set`: refused unless a person allows values over MCP; then the write in each
/// targeted view.
///
/// # Errors
///
/// `plugin.storage-panel.errors.mcp_values_off`, or arguments that do not read.
pub fn set(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    if !Settings::read(host).mcp_values {
        return Err(coded(FailureKind::AccessDenied, "errors.mcp_values_off"));
    }
    write_in(host, args)
}

/// A write in the views of `target` (shared by `storage_set` and the panel).
///
/// # Errors
///
/// Arguments that do not read.
pub fn write_in(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let target = target(args)?;
    let write = write::parse(args).map_err(|field| bad_args(&field))?;
    let columns = columns::columns(host);
    let results = write::run(host, &columns, target, &write);
    Ok(json!({
        "results": results,
        "$post": [{ "target": "panel", "message": { "type": "changed" } }],
    }))
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
