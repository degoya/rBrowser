//! The WebDriver commands rBrowser sends `safaridriver` (W3C WebDriver, classic): the driver's
//! status and new sessions ([`DriverClient`]), and per session - one Safari window - navigation,
//! history, scripts, input actions, the window's rectangle, screenshots, element lookup and the
//! end ([`Session`]). Every request goes through [`crate::http`] to `127.0.0.1`; ids from an
//! answer go into a path only when they are plain.

use std::time::Duration;

use serde_json::{Value, json};

use crate::error::SafariError;
use crate::http::{self, HttpError, Method, is_plain_segment};
use crate::protocol::{element_id, outcome};

/// How long a command waits for its answer.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a navigation waits: WebDriver answers it after the page loaded.
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// `safaridriver` on its port of `127.0.0.1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriverClient {
    port: u16,
}

impl DriverClient {
    /// The driver on `port`.
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Its port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Whether the driver answers and is ready for a new session (`GET /status`).
    pub async fn ready(&self) -> Result<bool, SafariError> {
        let value = self
            .call(Method::Get, "/status", None, "status", DEFAULT_TIMEOUT)
            .await?;
        Ok(value.get("ready").and_then(Value::as_bool).unwrap_or(false))
    }

    /// A new session - a new Safari window - with `capabilities` as `alwaysMatch`.
    pub async fn new_session(&self, capabilities: Value) -> Result<Session, SafariError> {
        let body = json!({ "capabilities": { "alwaysMatch": capabilities } });
        let value = self
            .call(
                Method::Post,
                "/session",
                Some(&body),
                "new session",
                DEFAULT_TIMEOUT,
            )
            .await?;
        let id = value
            .get("sessionId")
            .and_then(Value::as_str)
            .filter(|id| is_plain_segment(id))
            .ok_or_else(|| protocol("new session", "no usable session id"))?;
        let version = value
            .pointer("/capabilities/browserVersion")
            .and_then(Value::as_str)
            .map(|version| rb_util::text::cut(version, 32));
        Ok(Session {
            driver: self.clone(),
            id: id.to_owned(),
            version,
        })
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        command: &str,
        timeout: Duration,
    ) -> Result<Value, SafariError> {
        let answer = http::request(self.port, method, path, body, timeout)
            .await
            .map_err(|error| match error {
                HttpError::Connect(reason) | HttpError::Io(reason) => SafariError::Connect(reason),
                HttpError::Timeout(ms) => SafariError::Timeout {
                    command: command.to_owned(),
                    ms,
                },
                HttpError::Malformed(reason) => protocol(command, &reason),
            })?;
        outcome(command, answer)
    }
}

/// One WebDriver session: one Safari window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    driver: DriverClient,
    id: String,
    version: Option<String>,
}

impl Session {
    /// The session's id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The Safari version the session runs, as the driver named it.
    pub fn browser_version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    async fn call(
        &self,
        method: Method,
        rest: &str,
        body: Option<Value>,
        command: &str,
        timeout: Duration,
    ) -> Result<Value, SafariError> {
        let path = format!("/session/{}{rest}", self.id);
        self.driver
            .call(method, &path, body.as_ref(), command, timeout)
            .await
    }

    /// Loads `url` and waits for the load (`POST /url`).
    pub async fn navigate(&self, url: &str) -> Result<(), SafariError> {
        let body = json!({ "url": url });
        self.call(Method::Post, "/url", Some(body), "navigate", LOAD_TIMEOUT)
            .await
            .map(drop)
    }

    /// The URL of the top document (`GET /url`).
    pub async fn current_url(&self) -> Result<String, SafariError> {
        let value = self
            .call(Method::Get, "/url", None, "get url", DEFAULT_TIMEOUT)
            .await?;
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| protocol("get url", "the URL is not text"))
    }

    /// History back (`POST /back`).
    pub async fn back(&self) -> Result<(), SafariError> {
        self.history("/back", "back").await
    }

    /// History forward (`POST /forward`).
    pub async fn forward(&self) -> Result<(), SafariError> {
        self.history("/forward", "forward").await
    }

    /// Reloads (`POST /refresh`); WebDriver cannot bypass the cache.
    pub async fn refresh(&self) -> Result<(), SafariError> {
        self.history("/refresh", "refresh").await
    }

    async fn history(&self, rest: &str, command: &str) -> Result<(), SafariError> {
        self.call(Method::Post, rest, Some(json!({})), command, LOAD_TIMEOUT)
            .await
            .map(drop)
    }

    /// Runs `script` as a function body with `args` in the top document (`POST /execute/sync`)
    /// and answers its return value.
    pub async fn execute(&self, script: &str, args: Vec<Value>) -> Result<Value, SafariError> {
        let body = json!({ "script": script, "args": args });
        self.call(
            Method::Post,
            "/execute/sync",
            Some(body),
            "execute script",
            DEFAULT_TIMEOUT,
        )
        .await
    }

    /// Replays the input `sources` (`POST /actions`).
    pub async fn perform_actions(&self, sources: Vec<Value>) -> Result<(), SafariError> {
        let body = json!({ "actions": sources });
        self.call(
            Method::Post,
            "/actions",
            Some(body),
            "perform actions",
            DEFAULT_TIMEOUT,
        )
        .await
        .map(drop)
    }

    /// Releases every key and button held (`DELETE /actions`).
    pub async fn release_actions(&self) -> Result<(), SafariError> {
        self.call(
            Method::Delete,
            "/actions",
            None,
            "release actions",
            DEFAULT_TIMEOUT,
        )
        .await
        .map(drop)
    }

    /// Moves or sizes the window (`POST /window/rect`); `body` holds what changes.
    pub async fn set_window_rect(&self, body: Value) -> Result<(), SafariError> {
        self.call(
            Method::Post,
            "/window/rect",
            Some(body),
            "set window rect",
            DEFAULT_TIMEOUT,
        )
        .await
        .map(drop)
    }

    /// Brings the session's window to the front: `GET /window`, then `POST /window` with it.
    pub async fn focus_window(&self) -> Result<(), SafariError> {
        let handle = self
            .call(Method::Get, "/window", None, "get window", DEFAULT_TIMEOUT)
            .await?;
        let handle = handle
            .as_str()
            .ok_or_else(|| protocol("get window", "the window handle is not text"))?
            .to_owned();
        let body = json!({ "handle": handle });
        self.call(
            Method::Post,
            "/window",
            Some(body),
            "switch to window",
            DEFAULT_TIMEOUT,
        )
        .await
        .map(drop)
    }

    /// A PNG of the viewport, base64 (`GET /screenshot`).
    pub async fn screenshot(&self) -> Result<String, SafariError> {
        let value = self
            .call(
                Method::Get,
                "/screenshot",
                None,
                "take screenshot",
                DEFAULT_TIMEOUT,
            )
            .await?;
        png_of("take screenshot", &value)
    }

    /// The first element `selector` matches (`POST /element`, CSS).
    pub async fn find_css(&self, selector: &str) -> Result<String, SafariError> {
        let body = json!({ "using": "css selector", "value": selector });
        let value = self
            .call(
                Method::Post,
                "/element",
                Some(body),
                "find element",
                DEFAULT_TIMEOUT,
            )
            .await?;
        element_id(&value).ok_or_else(|| protocol("find element", "no usable element reference"))
    }

    /// A PNG of `element`, base64 (`GET /element/{id}/screenshot`).
    pub async fn element_screenshot(&self, element: &str) -> Result<String, SafariError> {
        if !is_plain_segment(element) {
            return Err(protocol("take element screenshot", "not an element id"));
        }
        let rest = format!("/element/{element}/screenshot");
        let value = self
            .call(
                Method::Get,
                &rest,
                None,
                "take element screenshot",
                DEFAULT_TIMEOUT,
            )
            .await?;
        png_of("take element screenshot", &value)
    }

    /// Ends the session: Safari closes its window (`DELETE /session/{id}`).
    pub async fn delete(&self) -> Result<(), SafariError> {
        self.call(Method::Delete, "", None, "delete session", DEFAULT_TIMEOUT)
            .await
            .map(drop)
    }
}

fn png_of(command: &str, value: &Value) -> Result<String, SafariError> {
    value
        .as_str()
        .filter(|png| !png.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| protocol(command, "the screenshot is not base64 text"))
}

fn protocol(command: &str, reason: &str) -> SafariError {
    SafariError::Protocol {
        command: command.to_owned(),
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
