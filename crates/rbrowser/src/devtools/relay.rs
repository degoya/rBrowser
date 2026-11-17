//! What passes between the docked frontend and the browser: the frontend talks to one page
//! target as if it had its own connection; the relay pins every message without a `sessionId`
//! to the page's session on the pipe, admits only the child sessions the page announced
//! (frames, workers it auto-attached), and answers with the frontend's own ids.
//!
//! A session attached over the pipe may reach the whole browser (`Target.getTargets`,
//! `Target.attachToTarget`, `Browser.close`), so the relay admits only what a frontend
//! inspecting one page needs: `Target.setAutoAttach`, `Target.setDiscoverTargets`,
//! `Target.getTargetInfo` and a flat `Target.attachToTarget` of its own target or of a child
//! the page announced (the Lighthouse panel's parallel session, RB-080-21),
//! `Target.detachFromTarget` of its own children and `Browser.getVersion`; everything else of
//! the `Target` and `Browser` domains is refused. Discovery tells it of those targets only, and
//! events about hidden targets ([`super::hidden`]) never reach it, nor do the contexts of
//! rBrowser's own isolated worlds ([`super::own_worlds`], RB-090-51).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use rb_engine::EngineError;
use serde_json::{Map, Value, json};

use super::hidden::is_hidden_target;
use super::own_worlds::OwnWorlds;
use crate::cdp::CdpEvent;

/// The error code of a refused or failed call (CDP's server error).
const SERVER_ERROR: i64 = -32000;
/// The error code of an unknown session (as Chromium answers it).
const NO_SESSION: i64 = -32001;
/// The error code of a message that is no call.
const INVALID_REQUEST: i64 = -32600;

/// One call of the frontend, to send on the pipe.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    /// The frontend's id, for the answer.
    pub id: Value,
    /// The method.
    pub method: String,
    /// Its parameters.
    pub params: Value,
    /// The session on the pipe: the page's or a child's.
    pub session: String,
    /// The child session the frontend named; the answer carries it back.
    pub child: Option<String>,
}

/// What a frontend message becomes.
#[derive(Clone, Debug, PartialEq)]
pub enum FromFrontend {
    /// A call to send.
    Call(Call),
    /// An answer for the frontend at once: refused or malformed.
    Answer(Value),
    /// Nothing at all: not even an id to answer.
    Drop,
}

/// What a browser event becomes.
#[derive(Clone, Debug, PartialEq)]
pub enum ToFrontend {
    /// The message for the frontend.
    Forward(Value),
    /// Not the frontend's.
    Skip,
    /// The page's session is gone: the connection ends.
    PageGone,
}

/// The state of one frontend connection.
#[derive(Debug)]
pub struct Relay {
    page: String,
    target: String,
    /// The child sessions the page announced, each with the target it was attached to.
    children: HashMap<String, Option<String>>,
    /// The browser answered the frontend's `Overlay.enable` on the page's session and no
    /// `Overlay.disable` followed: the frontend reveals a node named by
    /// `Overlay.inspectNodeRequested` (RB-080-43), as Chromium's own agent holds an inspect
    /// request back until the overlay is enabled.
    overlay_enabled: AtomicBool,
    /// The contexts of rBrowser's worlds kept from the Console.
    own_worlds: OwnWorlds,
}

impl Relay {
    /// A relay over the page's flat session `page` to the page target `target`.
    pub fn new(page: String, target: String) -> Self {
        Self {
            page,
            target,
            children: HashMap::new(),
            overlay_enabled: AtomicBool::new(false),
            own_worlds: OwnWorlds::default(),
        }
    }

    /// Whether the frontend has enabled the page's overlay domain, so it acts on
    /// [`Self::inspect_event`].
    pub fn overlay_enabled(&self) -> bool {
        self.overlay_enabled.load(Ordering::Acquire)
    }

    /// The browser answered `call` (`ok`: with a result, not an error): a successful
    /// `Overlay.enable` on the page's session turns the overlay on.
    pub fn on_answered(&self, call: &Call, ok: bool) {
        if ok && call.child.is_none() && call.method == "Overlay.enable" {
            self.overlay_enabled.store(true, Ordering::Release);
        }
    }

    /// The event that makes the frontend reveal and select the node `backend_node_id` in its
    /// Elements panel: what Chromium's overlay agent sends for "Inspect" (RB-080-43).
    pub fn inspect_event(backend_node_id: i64) -> Value {
        json!({
            "method": "Overlay.inspectNodeRequested",
            "params": { "backendNodeId": backend_node_id },
        })
    }

    /// Whether `target_id` is the frontend's own target or one of its announced children's.
    fn reaches(&self, target_id: &str) -> bool {
        target_id == self.target
            || self
                .children
                .values()
                .flatten()
                .any(|child| child == target_id)
    }

    /// The page's session on the pipe.
    pub fn page_session(&self) -> &str {
        &self.page
    }

    /// Reads one text message of the frontend.
    pub fn read_message(&self, text: &str) -> FromFrontend {
        let Ok(Value::Object(message)) = serde_json::from_str::<Value>(text) else {
            return FromFrontend::Drop;
        };
        let Some(id) = message.get("id").filter(|id| id.is_u64()).cloned() else {
            return FromFrontend::Drop;
        };
        let child = message
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return FromFrontend::Answer(error(
                &id,
                child.as_deref(),
                INVALID_REQUEST,
                "Invalid request",
            ));
        };
        let params = match message.get("params") {
            None | Some(Value::Null) => Value::Object(Map::new()),
            Some(params @ Value::Object(_)) => params.clone(),
            Some(_) => {
                return FromFrontend::Answer(error(
                    &id,
                    child.as_deref(),
                    INVALID_REQUEST,
                    "Invalid parameters",
                ));
            }
        };
        let session = match &child {
            None => self.page.clone(),
            Some(child) if self.children.contains_key(child) => child.clone(),
            Some(_) => {
                return FromFrontend::Answer(error(
                    &id,
                    child.as_deref(),
                    NO_SESSION,
                    "Session with given id not found.",
                ));
            }
        };
        if !self.admits(method, &params) {
            return FromFrontend::Answer(error(&id, child.as_deref(), SERVER_ERROR, "Not allowed"));
        }
        if child.is_none() && method == "Overlay.disable" {
            self.overlay_enabled.store(false, Ordering::Release);
        }
        FromFrontend::Call(Call {
            id,
            method: method.to_owned(),
            params,
            session,
            child,
        })
    }

    /// Whether a frontend inspecting one page may send `method`.
    fn admits(&self, method: &str, params: &Value) -> bool {
        if let Some(target) = method.strip_prefix("Target.") {
            let target_id = params.get("targetId").and_then(Value::as_str);
            return match target {
                "setAutoAttach" | "setDiscoverTargets" => true,
                "getTargetInfo" => target_id.is_none_or(|id| self.reaches(id)),
                "attachToTarget" => {
                    params.get("flatten").and_then(Value::as_bool) == Some(true)
                        && target_id.is_some_and(|id| self.reaches(id))
                }
                "detachFromTarget" => params
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .is_some_and(|session| self.children.contains_key(session)),
                _ => false,
            };
        }
        if let Some(browser) = method.strip_prefix("Browser.") {
            return browser == "getVersion";
        }
        true
    }

    /// The answer to `call`.
    pub fn answer(call: &Call, reply: Result<Value, EngineError>) -> Value {
        match reply {
            Ok(result) => {
                let mut message = json!({ "id": call.id, "result": result });
                if let Some(child) = &call.child {
                    message["sessionId"] = json!(child);
                }
                message
            }
            Err(EngineError::Cdp { code, message, .. }) => {
                let code = if code == 0 { SERVER_ERROR } else { code };
                error(&call.id, call.child.as_deref(), code, &message)
            }
            Err(other) => error(
                &call.id,
                call.child.as_deref(),
                SERVER_ERROR,
                &other.to_string(),
            ),
        }
    }

    /// Reads one browser event: the page's events lose their `sessionId`, a child's keep it,
    /// the rest is not the frontend's. Discovery (`Target.targetCreated`, `targetInfoChanged`,
    /// `targetDestroyed`) shows only the frontend's own target and its children.
    pub fn read_event(&mut self, event: &CdpEvent) -> ToFrontend {
        let Some(session) = event.session_id.as_deref() else {
            // A browser-level event: only the page's own detach concerns this connection.
            let detached = event.method == "Target.detachedFromTarget"
                && event.params.get("sessionId").and_then(Value::as_str) == Some(&self.page);
            return if detached {
                ToFrontend::PageGone
            } else {
                ToFrontend::Skip
            };
        };
        let own = session == self.page;
        if !own && !self.children.contains_key(session) {
            return ToFrontend::Skip;
        }
        let info = event.params.get("targetInfo");
        if info.is_some_and(is_hidden_target)
            || self.own_worlds.hides(session, &event.method, &event.params)
        {
            return ToFrontend::Skip;
        }
        let target_id = event
            .params
            .get("targetId")
            .or_else(|| info.and_then(|info| info.get("targetId")))
            .and_then(Value::as_str);
        let announced = event.params.get("sessionId").and_then(Value::as_str);
        match (event.method.as_str(), announced) {
            ("Target.attachedToTarget", Some(child)) => {
                self.children
                    .insert(child.to_owned(), target_id.map(str::to_owned));
            }
            ("Target.detachedFromTarget", Some(child)) => {
                self.children.remove(child);
                self.own_worlds.forget(child);
            }
            ("Target.targetCreated" | "Target.targetInfoChanged" | "Target.targetDestroyed", _)
                if !target_id.is_some_and(|id| self.reaches(id)) =>
            {
                return ToFrontend::Skip;
            }
            _ => {}
        }
        let mut message = json!({ "method": event.method, "params": event.params });
        if !own {
            message["sessionId"] = json!(session);
        }
        ToFrontend::Forward(message)
    }
}

fn error(id: &Value, child: Option<&str>, code: i64, text: &str) -> Value {
    let mut message = json!({ "id": id, "error": { "code": code, "message": text } });
    if let Some(child) = child {
        message["sessionId"] = json!(child);
    }
    message
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
