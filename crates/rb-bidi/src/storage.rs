//! `storage.setCookie`: a cookie into Firefox's jar, for copying cookies from Chromium
//! (RB-040-05). The value is a secret: it travels only in the command, whose `Debug` hides it.
//!
//! Sessions (RB-020-07): the `default` session is Firefox's default user context; each named
//! session gets a user context of its own (`browser.createUserContext`), so its views and cookies
//! never mix with another session's - as Chromium's request contexts keep them apart.

use serde_json::{Value, json};

use crate::error::BidiError;
use crate::protocol::Command;

/// The session every view is in unless its breakpoint names another (`rb_config`'s `default`).
pub const DEFAULT_SESSION: &str = "default";

/// One cookie to set.
#[derive(Clone, PartialEq, Eq)]
pub struct Cookie {
    /// The name.
    pub name: String,
    /// The value; never logged.
    pub value: String,
    /// The domain (`localhost`, `.example.com`).
    pub domain: String,
    /// The path.
    pub path: String,
    /// `HttpOnly`.
    pub http_only: bool,
    /// `Secure`.
    pub secure: bool,
    /// `strict`, `lax` or `none`.
    pub same_site: String,
    /// Seconds since the epoch; `None` for a session cookie.
    pub expiry: Option<u64>,
}

impl std::fmt::Debug for Cookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cookie")
            .field("name", &self.name)
            .field("value", &"<masked>")
            .field("domain", &self.domain)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// Sets `cookie` in the default user context.
pub fn set_cookie(cookie: &Cookie) -> Command {
    set_cookie_in(cookie, None)
}

/// Sets `cookie` in `user_context`; the default user context with `None`.
pub fn set_cookie_in(cookie: &Cookie, user_context: Option<&str>) -> Command {
    let mut value = json!({
        "name": cookie.name,
        "value": { "type": "string", "value": cookie.value },
        "domain": cookie.domain,
        "path": cookie.path,
        "httpOnly": cookie.http_only,
        "secure": cookie.secure,
        "sameSite": cookie.same_site,
    });
    if let (Some(expiry), Value::Object(fields)) = (cookie.expiry, &mut value) {
        fields.insert("expiry".to_owned(), json!(expiry));
    }
    let mut params = json!({ "cookie": value });
    if let (Some(user_context), Value::Object(fields)) = (user_context, &mut params) {
        fields.insert(
            "partition".to_owned(),
            json!({ "type": "storageKey", "userContext": user_context }),
        );
    }
    Command::call("storage.setCookie", params)
}

/// A new user context: a cookie jar and storage of its own, for a named session.
pub fn create_user_context() -> Command {
    Command::call("browser.createUserContext", json!({}))
}

/// The id of a `browser.createUserContext` answer.
pub fn user_context_of(result: &Value) -> Result<String, BidiError> {
    result
        .get("userContext")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| BidiError::Protocol {
            method: "browser.createUserContext".to_owned(),
            reason: "no userContext".to_owned(),
        })
}

/// Whether `session` is the default session (Firefox's default user context); an empty id is.
pub fn is_default_session(session: &str) -> bool {
    session.is_empty() || session == DEFAULT_SESSION
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
