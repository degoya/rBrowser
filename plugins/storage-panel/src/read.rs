//! Reading a view: its cookies through CDP `Network.getCookies` for the view's own URL - the
//! cookies its page is sent, HttpOnly ones included, never another site's (no `getAllCookies`) -
//! and its localStorage and sessionStorage through the operations of `src/ops.js`, run with
//! `evaluate` in the plugin's world of the main frame.

use rb_plugin_api::{Failure, PluginHost};
use serde_json::{Value, json};

use crate::columns::{Column, Target};
use crate::cookie::{self, Cookie};

/// The storage operations, a JavaScript function of one JSON argument.
pub const OPS: &str = include_str!("ops.js");
/// The most entries read from one storage area of one view.
pub const MAX_ITEMS: usize = 1000;

/// What the plugin keeps apart: the cookie jar and the two storage areas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Cookies,
    Local,
    Session,
}

impl Kind {
    /// Every kind.
    pub const ALL: [Self; 3] = [Self::Cookies, Self::Local, Self::Session];

    /// The kind `name` means: `cookies`/`cookie`, `local`/`localStorage`,
    /// `session`/`sessionStorage`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "cookies" | "cookie" => Some(Self::Cookies),
            "local" | "localStorage" => Some(Self::Local),
            "session" | "sessionStorage" => Some(Self::Session),
            _ => None,
        }
    }

    /// The name MCP answers use.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Cookies => "cookies",
            Self::Local => "localStorage",
            Self::Session => "sessionStorage",
        }
    }

    /// The storage area `src/ops.js` takes (`local` for the cookies, which it never sees).
    #[must_use]
    pub fn area(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Cookies | Self::Local => "local",
        }
    }
}

/// `evaluate`'s script running `op` through [`OPS`].
#[must_use]
pub fn script(op: &Value) -> String {
    format!("({})({op})", OPS.trim())
}

/// What one view holds.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewData {
    pub column: Column,
    /// `ok`, `unsupported` (Firefox, WebKit) or `error`.
    pub status: &'static str,
    /// The failure code of a view that could not be read.
    pub error: Option<String>,
    pub cookies: Vec<Cookie>,
    /// The origin the document reports; `None` when the storage was not read.
    pub origin: Option<String>,
    pub local: Vec<(String, String)>,
    pub session: Vec<(String, String)>,
    /// Storage entries beyond [`MAX_ITEMS`] that were not read.
    pub more: u64,
    /// Why the storage could not be read (`unavailable`: an opaque origin, storage off).
    pub storage_error: Option<String>,
}

impl ViewData {
    fn new(column: &Column, status: &'static str) -> Self {
        Self {
            column: column.clone(),
            status,
            error: None,
            cookies: Vec::new(),
            origin: None,
            local: Vec::new(),
            session: Vec::new(),
            more: 0,
            storage_error: None,
        }
    }

    /// The entries of storage `kind`.
    #[must_use]
    pub fn storage(&self, kind: Kind) -> &[(String, String)] {
        match kind {
            Kind::Session => &self.session,
            Kind::Local | Kind::Cookies => &self.local,
        }
    }
}

fn failed(column: &Column, failure: &Failure) -> ViewData {
    let mut data = ViewData::new(column, "error");
    data.error = Some(
        failure
            .code
            .clone()
            .unwrap_or_else(|| failure.kind.as_str().to_owned()),
    );
    data
}

fn pairs(value: &Value) -> Vec<(String, String)> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|pair| {
                    Some((
                        pair.get(0)?.as_str()?.to_owned(),
                        pair.get(1)?.as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Reads `kinds` of `column`; HttpOnly cookies only with `http_only`.
pub fn read_view(
    host: &mut dyn PluginHost,
    column: &Column,
    kinds: &[Kind],
    http_only: bool,
) -> ViewData {
    if !column.supported() {
        return ViewData::new(column, "unsupported");
    }
    let mut data = ViewData::new(column, "ok");
    // A page without a web origin (about:blank) has no cookies to show.
    if kinds.contains(&Kind::Cookies) && column.origin.is_some() {
        let params = json!({ "urls": [column.url] }).to_string();
        match host.cdp_send(column.id, "Network.getCookies", &params) {
            Ok(answer) => match cookie::parse_cookies(&answer) {
                Ok(cookies) => {
                    data.cookies = cookies
                        .into_iter()
                        .filter(|cookie| http_only || !cookie.http_only)
                        .collect();
                }
                Err(_) => data.error = Some("plugins.error.bad_output".to_owned()),
            },
            Err(failure) => return failed(column, &failure),
        }
    }
    let local = kinds.contains(&Kind::Local);
    let session = kinds.contains(&Kind::Session);
    if local || session {
        let op = json!({ "op": "read", "local": local, "session": session, "max": MAX_ITEMS });
        match host.evaluate(column.id, &script(&op)) {
            Ok(answer) => {
                let value: Value = serde_json::from_str(&answer).unwrap_or(Value::Null);
                data.origin = value["origin"]
                    .as_str()
                    .filter(|origin| *origin != "null")
                    .map(str::to_owned);
                if let Some(reason) = value["reason"].as_str() {
                    data.storage_error = Some(reason.to_owned());
                }
                data.local = pairs(&value["local"]);
                data.session = pairs(&value["session"]);
                data.more = value["more"].as_u64().unwrap_or(0);
            }
            Err(failure) => return failed(column, &failure),
        }
    }
    if data.error.is_some() {
        data.status = "error";
    }
    data
}

/// Reads `kinds` of every column `target` covers.
pub fn read_all(
    host: &mut dyn PluginHost,
    columns: &[Column],
    target: Target,
    kinds: &[Kind],
    http_only: bool,
) -> Vec<ViewData> {
    columns
        .iter()
        .filter(|column| target.covers(column))
        .map(|column| read_view(host, column, kinds, http_only))
        .collect()
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
