//! The environment comparison (the review condition of RB-093-03, what Lighthouse does not
//! give): the same page in each environment, side by side - a table of header x environment,
//! a row per checked header and per cookie name, `differs` where the environments disagree
//! ("prod has HSTS, staging not").
//!
//! A cell is the header's value as received (`null` when absent), its whitespace collapsed, a
//! CSP nonce masked (`'nonce-*'`, it changes with every answer) and cut to [`MAX_CELL_CHARS`]
//! characters; a cookie's cell lists its attributes ([`crate::cookies::summary`]), never its
//! value (empty when it has none). An environment whose page could not be fetched shows its failure and takes no part in
//! `differs`.

use rb_plugin_api::{Failure, Finding};
use serde_json::{Map, Value, json};

use crate::cookies::{self, Cookie};
use crate::headers::{Headers, shown};
use crate::rules;

/// The headers the table compares, as the rules read them.
pub const COMPARED: [&str; 8] = [
    "content-security-policy",
    "strict-transport-security",
    "x-frame-options",
    "x-content-type-options",
    "referrer-policy",
    "permissions-policy",
    "access-control-allow-origin",
    "access-control-allow-credentials",
];

/// The longest cell, in characters.
pub const MAX_CELL_CHARS: usize = 512;

/// One environment's page, checked.
#[derive(Clone, Debug)]
pub struct Audited {
    pub final_url: String,
    pub status: u16,
    pub headers: Headers,
    pub cookies: Vec<Cookie>,
    pub findings: Vec<Finding>,
}

/// One column of the table: an environment and what its page answered.
#[derive(Clone, Debug)]
pub struct Column {
    pub id: String,
    pub url: String,
    pub outcome: Result<Audited, Failure>,
}

/// `value` as a cell shows it.
#[must_use]
pub fn normalize(value: &str) -> String {
    let words: Vec<String> = value
        .split_ascii_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            match lower.strip_prefix("'nonce-") {
                Some(rest) => {
                    let tail = rest.find('\'').and_then(|end| rest.get(end + 1..));
                    format!("'nonce-*'{}", tail.unwrap_or_default())
                }
                None => word.to_owned(),
            }
        })
        .collect();
    shown(&words.join(" "), MAX_CELL_CHARS)
}

/// One row: its cells by environment id and whether the fetched environments differ.
fn row(label: Value, columns: &[Column], cell: impl Fn(&Audited) -> Option<String>) -> Value {
    let mut values = Map::new();
    let mut seen: Vec<Option<String>> = Vec::new();
    for column in columns {
        let value = column.outcome.as_ref().ok().map(&cell);
        if let Some(value) = &value {
            seen.push(value.clone());
        }
        values.insert(column.id.clone(), json!(value.flatten()));
    }
    let differs = seen.windows(2).any(|pair| pair[0] != pair[1]);
    let mut row = label;
    row["values"] = Value::Object(values);
    row["differs"] = json!(differs);
    row
}

fn column_json(column: &Column) -> Value {
    match &column.outcome {
        Ok(audited) => json!({
            "id": column.id,
            "url": column.url,
            "finalUrl": audited.final_url,
            "status": audited.status,
            "findings": audited.findings.iter().map(rules::finding_json).collect::<Vec<_>>(),
        }),
        Err(failure) => json!({
            "id": column.id,
            "url": column.url,
            "error": { "code": failure.code, "params": failure.params },
        }),
    }
}

/// The table of `columns`: `environments` (each with its findings or its failure), `rows`
/// (`header`, for a cookie also `cookie`, `values` by environment id, `differs`) and the count
/// of `differences`.
#[must_use]
pub fn table(path: &str, columns: &[Column]) -> Value {
    let mut rows: Vec<Value> = COMPARED
        .iter()
        .map(|name| {
            row(json!({ "header": name }), columns, |audited| {
                audited.headers.get(name).map(|value| normalize(&value))
            })
        })
        .collect();
    let mut names: Vec<String> = Vec::new();
    for audited in columns
        .iter()
        .filter_map(|column| column.outcome.as_ref().ok())
    {
        for cookie in &audited.cookies {
            if names.len() < cookies::MAX_COOKIES && !names.contains(&cookie.name) {
                names.push(cookie.name.clone());
            }
        }
    }
    for name in &names {
        rows.push(row(
            json!({ "header": "set-cookie", "cookie": name }),
            columns,
            |audited| {
                audited
                    .cookies
                    .iter()
                    .find(|cookie| cookie.name == *name)
                    .map(cookies::summary)
            },
        ));
    }
    let differences = rows
        .iter()
        .filter(|row| row["differs"] == json!(true))
        .count();
    json!({
        "path": path,
        "environments": columns.iter().map(column_json).collect::<Vec<_>>(),
        "rows": rows,
        "differences": differences,
    })
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
