//! The command `panel`: without an `op` (the shortcut, the palette) it asks the host to show
//! the panel; the panel itself calls it with an `op` - a person's path, never MCP's, so it sees
//! and edits values whatever `storage-panel.mcpValues` says:
//!
//! - `list` (`kind`, `origin`, `search`): the views as columns and the rows of one kind, values
//!   cut to [`CELL_CHARS`], the answer kept under the panel bridge's 64 KiB;
//! - `value` (`kind`, `view`, `scope`, `name`, `path`, `partition`): one cell's whole value;
//! - `set`, `delete` (`kind`, `target`, `name`, `value`, `origin`, `cookie`): a write;
//! - `pin` (`kind`, `view`, `scope`, `name`, `path`, `partition`, `sessionOnly`), `unpin` (`id`),
//!   `enable` (`id`, `enabled`), `pins`: the pinned values, applied to every view at once.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId};
use serde_json::{Value, json};

use crate::columns::{self, Column, Target};
use crate::matrix::{self, Row, Values};
use crate::pins::{self, MAX_PINS, Pin};
use crate::read::{self, Kind, ViewData};
use crate::settings::Settings;
use crate::tools::{self, Filter};
use crate::{OPEN_KEY, PANEL_ID, bad_args, coded};

/// The characters of a value a cell shows.
pub const CELL_CHARS: usize = 120;
/// The most bytes of rows one `list` answers.
pub const LIST_BUDGET: usize = 52 * 1024;
/// The most characters `value` answers.
pub const VALUE_CHARS: usize = 48 * 1024;

fn changed() -> Value {
    json!([{ "target": "panel", "message": { "type": "changed" } }])
}

fn one_kind(args: &Value) -> Result<Kind, Failure> {
    args["kind"]
        .as_str()
        .and_then(Kind::parse)
        .ok_or_else(|| bad_args("kind"))
}

fn view_column(columns: &[Column], args: &Value) -> Result<Column, Failure> {
    let id = args["view"]
        .as_u64()
        .and_then(|id| ViewId::try_from(id).ok())
        .ok_or_else(|| bad_args("view"))?;
    columns
        .iter()
        .find(|column| column.id == id)
        .cloned()
        .ok_or_else(|| coded(FailureKind::NotFound, "errors.unknown_view").with_param("view", id))
}

fn column_json(data: &ViewData) -> Value {
    let mut out = data.column.to_json(data.status);
    out["error"] = json!(data.error);
    out["storageError"] = json!(data.storage_error);
    out["documentOrigin"] = json!(data.origin);
    out["notRead"] = json!(data.more);
    out
}

/// `list`: the columns and the rows of one kind.
///
/// # Errors
///
/// Arguments that do not read.
pub fn list(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let kind = one_kind(args)?;
    let filter = Filter::parse(&json!({ "kind": kind.name(), "origin": args["origin"],
                                        "search": args["search"] }))?;
    let columns = columns::columns(host);
    let data = read::read_all(
        host,
        &columns,
        Target::All,
        &[kind],
        settings.show_http_only,
    );
    let pins = pins::load(host);
    let all: Vec<Row> = matrix::rows(kind, &data)
        .into_iter()
        .filter(|row| filter.passes(&row.scope, &row.name))
        .collect();
    let mut rows = Vec::new();
    let mut used = 0;
    for row in &all {
        let pinned = pins.iter().any(|pin| pin.holds(row));
        let json = matrix::row_json(row, Values::Shown(CELL_CHARS), pinned);
        used += json.to_string().len();
        if used > LIST_BUDGET {
            break;
        }
        rows.push(json);
    }
    Ok(json!({
        "kind": kind.name(),
        "views": data.iter().map(column_json).collect::<Vec<_>>(),
        "total": all.len(),
        "shown": rows.len(),
        "truncated": rows.len() < all.len(),
        "rows": rows,
    }))
}

/// The entry of one view a call names (`kind`, `view`, `scope`, `name`, `path`, `partition`).
fn find(host: &mut dyn PluginHost, args: &Value) -> Result<(Column, Row), Failure> {
    let kind = one_kind(args)?;
    let columns = columns::columns(host);
    let column = view_column(&columns, args)?;
    let data = read::read_view(host, &column, &[kind], true);
    let text = |key: &str| args[key].as_str().unwrap_or_default();
    matrix::rows(kind, std::slice::from_ref(&data))
        .into_iter()
        .find(|row| {
            row.scope == text("scope")
                && row.name == text("name")
                && row.path == text("path")
                && row.partition == text("partition")
        })
        .filter(|row| row.cells.get(&column.id).is_some_and(Option::is_some))
        .map(|row| (column, row))
        .ok_or_else(|| coded(FailureKind::NotFound, "errors.no_entry"))
}

/// `value`: one cell's whole value (up to [`VALUE_CHARS`]).
///
/// # Errors
///
/// Arguments that do not read; `plugin.storage-panel.errors.no_entry`.
pub fn value(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let (column, row) = find(host, args)?;
    let cell = row
        .cells
        .get(&column.id)
        .cloned()
        .flatten()
        .map(|cell| cell.value)
        .unwrap_or_default();
    Ok(Value::Object(matrix::value_json(
        &cell,
        Values::Shown(VALUE_CHARS),
    )))
}

/// The pins as the panel lists them (values cut).
fn pins_json(pins: &[Pin]) -> Value {
    json!(
        pins.iter()
            .map(|pin| {
                let mut out = pin.to_json();
                if let Some(fields) = out.as_object_mut() {
                    fields.extend(matrix::value_json(&pin.value, Values::Shown(CELL_CHARS)));
                }
                out
            })
            .collect::<Vec<_>>()
    )
}

/// Saves `pins`, applies them to every view and answers them with the views that failed.
fn store(host: &mut dyn PluginHost, pins: &[Pin]) -> Result<Value, Failure> {
    pins::save(host, pins)?;
    let enabled = Settings::read(host).pins_enabled;
    let applied: &[Pin] = if enabled { pins } else { &[] };
    let mut failed = Vec::new();
    for column in columns::columns(host) {
        if !pins::apply_view(host, &column, applied).is_empty() {
            failed.push(column.id);
        }
    }
    Ok(json!({ "pins": pins_json(pins), "failed": failed, "$post": changed() }))
}

/// `pin`: the entry of one view becomes a pin (of every session, or the view's with
/// `sessionOnly`).
///
/// # Errors
///
/// Arguments that do not read, no such entry, too many pins, a pins file that cannot be written,
/// an untrusted workspace.
pub fn pin(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let (column, row) = find(host, args)?;
    let Some(Some(cell)) = row.cells.get(&column.id).cloned() else {
        return Err(coded(FailureKind::NotFound, "errors.no_entry"));
    };
    let origin = match row.kind {
        Kind::Cookies => column.origin.clone(),
        Kind::Local | Kind::Session => Some(row.scope.clone()),
    }
    .ok_or_else(|| coded(FailureKind::NotSupported, "errors.no_origin"))?;
    let session = match &args["sessionOnly"] {
        Value::Bool(true) => Some(column.session.clone()),
        Value::Null | Value::Bool(false) => None,
        _ => return Err(bad_args("sessionOnly")),
    };
    // A pinned cookie is set again as a session cookie: its expiry is not kept.
    let cookie = cell.cookie.as_ref().map_or(Value::Null, |cookie| {
        json!({ "domain": cookie.domain, "path": cookie.path, "secure": cookie.secure,
                "httpOnly": cookie.http_only, "sameSite": cookie.same_site,
                "partition": cookie.partition })
    });
    let pin = Pin::parse(
        &json!({ "kind": row.kind.name(), "origin": origin, "key": row.name,
                                  "value": cell.value, "session": session, "cookie": cookie }),
    )
    .map_err(|field| coded(FailureKind::Limit, "errors.cannot_pin").with_param("field", field))?;
    let mut pins = pins::load_trusted(host)?;
    pins.retain(|old| old.id() != pin.id());
    if pins.len() >= MAX_PINS {
        return Err(coded(FailureKind::Limit, "errors.too_many_pins").with_param("limit", MAX_PINS));
    }
    pins.push(pin);
    store(host, &pins)
}

fn pin_id(args: &Value) -> Result<String, Failure> {
    args["id"]
        .as_str()
        .filter(|id| id.len() == 16 && id.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_owned)
        .ok_or_else(|| bad_args("id"))
}

/// `unpin` and `enable`: a pin removed or switched.
///
/// # Errors
///
/// Arguments that do not read, an unknown pin, a pins file that cannot be written, an untrusted
/// workspace.
pub fn change_pin(host: &mut dyn PluginHost, args: &Value, remove: bool) -> Result<Value, Failure> {
    let id = pin_id(args)?;
    let mut pins = pins::load_trusted(host)?;
    let Some(at) = pins.iter().position(|pin| pin.id() == id) else {
        return Err(coded(FailureKind::NotFound, "errors.no_pin"));
    };
    if remove {
        pins.remove(at);
    } else {
        pins[at].enabled = args["enabled"]
            .as_bool()
            .ok_or_else(|| bad_args("enabled"))?;
    }
    store(host, &pins)
}

/// The command `panel`.
///
/// # Errors
///
/// The op's.
pub fn command(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    match args["op"].as_str() {
        None if args["op"].is_null() => Ok(json!({ OPEN_KEY: { "panel": PANEL_ID } })),
        Some("list") => list(host, args),
        Some("value") => value(host, args),
        Some("set") => tools::write_in(host, args),
        Some("delete") => {
            let mut args = args.clone();
            args["delete"] = json!(true);
            tools::write_in(host, &args)
        }
        Some("pin") => pin(host, args),
        Some("unpin") => change_pin(host, args, true),
        Some("enable") => change_pin(host, args, false),
        Some("pins") => {
            let loaded = pins::load_trusted(host);
            let untrusted = loaded.is_err();
            let pins = loaded.unwrap_or_default();
            Ok(json!({ "pins": pins_json(&pins), "untrusted": untrusted }))
        }
        _ => Err(bad_args("op")),
    }
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
