//! Writing: one cookie or storage key set or removed in the views a call targets, each view
//! answering for itself. A cookie goes through CDP (`Network.setCookie`, `deleteCookies`), a
//! storage key through `src/ops.js` in the view's document - only when the document is of the
//! origin the write names (each view's own origin when it names none). Views of one session
//! share their cookies and localStorage, so a write there shows in each of them.

use rb_plugin_api::PluginHost;
use serde_json::{Value, json};

use crate::columns::{Column, Target};
use crate::cookie::{self, CookieSpec};
use crate::origin;
use crate::read::{self, Kind};

/// The longest storage key, in characters.
pub const MAX_KEY_CHARS: usize = 4096;
/// The longest storage value written, in characters.
pub const MAX_VALUE_CHARS: usize = 256 * 1024;

/// One write, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Write {
    pub kind: Kind,
    pub name: String,
    /// The value to set; `None` removes the entry.
    pub value: Option<String>,
    /// The storage origin meant; `None`: each view's own.
    pub origin: Option<String>,
    /// The cookie (for a removal its value is empty).
    pub cookie: Option<CookieSpec>,
}

/// The write `args` describe: `kind`, `name`, `value` or `delete: true`, `origin` (storage)
/// and `cookie` (`domain`, `path`, `expires`, `httpOnly`, `secure`, `sameSite`, `partition`).
///
/// # Errors
///
/// The name of the field that does not hold.
pub fn parse(args: &Value) -> Result<Write, String> {
    let kind = args["kind"]
        .as_str()
        .and_then(Kind::parse)
        .ok_or_else(|| "kind".to_owned())?;
    let name = args["name"]
        .as_str()
        .filter(|name| !name.is_empty() && name.chars().count() <= MAX_KEY_CHARS)
        .ok_or_else(|| "name".to_owned())?;
    let delete = match &args["delete"] {
        Value::Null => false,
        Value::Bool(delete) => *delete,
        _ => return Err("delete".to_owned()),
    };
    let value = match (&args["value"], delete) {
        (_, true) => None,
        (Value::String(value), false) if value.chars().count() <= MAX_VALUE_CHARS => {
            Some(value.clone())
        }
        _ => return Err("value".to_owned()),
    };
    let origin = match &args["origin"] {
        Value::Null => None,
        Value::String(origin) if origin::is_origin(origin) => Some(origin.clone()),
        _ => return Err("origin".to_owned()),
    };
    let cookie = if kind == Kind::Cookies {
        Some(cookie::spec(
            name,
            value.as_deref().unwrap_or_default(),
            &args["cookie"],
        )?)
    } else {
        None
    };
    Ok(Write {
        kind,
        name: name.to_owned(),
        value,
        origin,
        cookie,
    })
}

fn failed(view: u32, error: &str) -> Value {
    json!({ "view": view, "ok": false, "error": error })
}

fn error_key(name: &str) -> String {
    crate::key(&format!("errors.{name}"))
}

fn write_cookie(host: &mut dyn PluginHost, column: &Column, write: &Write) -> Value {
    let Some(spec) = &write.cookie else {
        return failed(column.id, &error_key("bad_args"));
    };
    // Only a cookie the view's page is sent: its host, or a domain the host belongs to.
    let Some(view_host) = column.origin.as_deref().and_then(origin::host_of) else {
        return failed(column.id, &error_key("no_origin"));
    };
    if spec
        .domain
        .as_deref()
        .is_some_and(|domain| !origin::domain_matches(&view_host, domain))
    {
        return failed(column.id, &error_key("other_origin"));
    }
    let (method, params) = match &write.value {
        Some(_) => ("Network.setCookie", cookie::set_params(spec, &column.url)),
        None => (
            "Network.deleteCookies",
            cookie::delete_params(
                &spec.name,
                spec.domain.as_deref(),
                &spec.path,
                spec.partition.as_deref(),
                &column.url,
            ),
        ),
    };
    match host.cdp_send(column.id, method, &params.to_string()) {
        Ok(answer) => {
            let answer: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
            if answer["success"] == false {
                failed(column.id, &error_key("rejected"))
            } else {
                json!({ "view": column.id, "ok": true })
            }
        }
        Err(failure) => failed(column.id, failure.code.as_deref().unwrap_or_default()),
    }
}

fn write_storage(host: &mut dyn PluginHost, column: &Column, write: &Write) -> Value {
    let Some(origin) = write.origin.clone().or_else(|| column.origin.clone()) else {
        return failed(column.id, &error_key("no_origin"));
    };
    let op = match &write.value {
        Some(value) => json!({ "op": "set", "area": write.kind.area(), "origin": origin,
                               "key": write.name, "value": value }),
        None => json!({ "op": "remove", "area": write.kind.area(), "origin": origin,
                        "key": write.name }),
    };
    match host.evaluate(column.id, &read::script(&op)) {
        Ok(answer) => {
            let answer: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
            match (answer["ok"].as_bool(), answer["reason"].as_str()) {
                (Some(true), _) => json!({ "view": column.id, "ok": true }),
                (_, Some("origin")) => failed(column.id, &error_key("other_origin")),
                (_, Some("quota")) => failed(column.id, &error_key("quota")),
                _ => failed(column.id, &error_key("unavailable")),
            }
        }
        Err(failure) => failed(column.id, failure.code.as_deref().unwrap_or_default()),
    }
}

/// Carries out `write` in every column `target` covers; one result per view, in grid order:
/// `{"view", "ok": true}` or `{"view", "ok": false, "error": <code>}`.
pub fn run(
    host: &mut dyn PluginHost,
    columns: &[Column],
    target: Target,
    write: &Write,
) -> Vec<Value> {
    columns
        .iter()
        .filter(|column| target.covers(column))
        .map(|column| {
            if !column.supported() {
                failed(column.id, &error_key("unsupported_engine"))
            } else if write.kind == Kind::Cookies {
                write_cookie(host, column, write)
            } else {
                write_storage(host, column, write)
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
