//! A scripted browser end for the tests: answers `Target.getTargets`, `Target.getTargetInfo`
//! (an error for an unknown id; without one, the session's own target), `Target.attachToTarget`
//! and `Target.attachToBrowserTarget` from a target list, `Page.addScriptToEvaluateOnNewDocument`
//! with a numbered identifier, `Browser.getVersion` with [`FAKE_USER_AGENT`], everything else with
//! `{}`, and records every message it was sent. A method can be made to fail
//! ([`FakeBrowser::fail`]) or to hold its answers until [`FakeBrowser::release`].
//!
//! With [`FakeBrowser::auto_attach`] it plays as much of a page as Playwright's
//! `connectOverCDP` needs (RB-040-06): `Target.setAutoAttach` on a browser session announces
//! every target (`Target.attachedToTarget`, in the default context), a page session answers
//! `Page.getFrameTree`, and `Page.navigate` commits the new URL with its lifecycle events.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use rb_engine::EngineError;
use serde_json::{Value, json};

use super::router::lock;
use super::{CdpRouter, CdpTransport};

/// The engine's user agent the fake reports.
pub(crate) const FAKE_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.6778.86 Safari/537.36";

#[derive(Default)]
pub(crate) struct FakeBrowser {
    router: Mutex<Weak<CdpRouter>>,
    sent: Mutex<Vec<Value>>,
    targets: Mutex<Vec<(String, String)>>,
    sessions: AtomicU64,
    failing: Mutex<Vec<String>>,
    canned: Mutex<Vec<(String, Value)>>,
    held: Mutex<Option<(String, Option<String>)>>,
    /// The held answers, each with the events scripted around it, in order.
    parked: Mutex<Vec<Vec<Value>>>,
    /// Session id to its target, for every page session handed out.
    pages: Mutex<BTreeMap<String, String>>,
    auto_attach: AtomicBool,
    loaders: AtomicU64,
}

/// Messages to deliver around the reply.
#[derive(Default)]
struct Script {
    before: Vec<Value>,
    after: Vec<Value>,
}

impl FakeBrowser {
    /// A fake and the router that talks to it.
    pub(crate) fn connect() -> (Arc<Self>, Arc<CdpRouter>) {
        let fake = Arc::new(Self::default());
        let router = CdpRouter::new(fake.clone());
        *lock(&fake.router) = Arc::downgrade(&router);
        (fake, router)
    }

    /// Plays the pages for Playwright: auto-attach, frame tree, navigation.
    pub(crate) fn auto_attach(&self) {
        self.auto_attach.store(true, Ordering::Relaxed);
    }

    pub(crate) fn add_target(&self, target_id: &str, url: &str) {
        lock(&self.targets).push((target_id.to_owned(), url.to_owned()));
    }

    /// The target of a page session this fake handed out.
    pub(crate) fn target_of(&self, session: &str) -> Option<String> {
        lock(&self.pages).get(session).cloned()
    }

    /// Every message sent with `method`, in order.
    pub(crate) fn sent(&self, method: &str) -> Vec<Value> {
        lock(&self.sent)
            .iter()
            .filter(|message| message["method"] == method)
            .cloned()
            .collect()
    }

    /// `method` fails from now on (`on`), or answers again.
    pub(crate) fn fail(&self, method: &str, on: bool) {
        let mut failing = lock(&self.failing);
        failing.retain(|failed| failed != method);
        if on {
            failing.push(method.to_owned());
        }
    }

    /// `method` answers `result` from now on.
    pub(crate) fn answer_with(&self, method: &str, result: Value) {
        lock(&self.canned).push((method.to_owned(), result));
    }

    /// The answers to `method` - in `session` only, or in every one - wait from now on until
    /// [`FakeBrowser::release`] (a hung page).
    pub(crate) fn hold(&self, method: &str, session: Option<&str>) {
        *lock(&self.held) = Some((method.to_owned(), session.map(str::to_owned)));
    }

    /// Delivers the answers held so far with their events, in order, and holds no more.
    pub(crate) fn release(&self) {
        *lock(&self.held) = None;
        let parked = std::mem::take(&mut *lock(&self.parked));
        if let Some(router) = lock(&self.router).upgrade() {
            for message in parked.into_iter().flatten() {
                router.on_message(message);
            }
        }
    }

    /// Delivers an event as the browser would.
    pub(crate) fn emit(&self, message: Value) {
        if let Some(router) = lock(&self.router).upgrade() {
            router.on_message(message);
        }
    }

    fn url_of(&self, target: &str) -> Option<String> {
        lock(&self.targets)
            .iter()
            .find(|(id, _)| id == target)
            .map(|(_, url)| url.clone())
    }

    fn new_session(&self, prefix: &str, target: Option<&str>) -> String {
        let n = self.sessions.fetch_add(1, Ordering::Relaxed) + 1;
        let session = format!("{prefix}{n}");
        if let Some(target) = target {
            lock(&self.pages).insert(session.clone(), target.to_owned());
        }
        session
    }

    fn playing(&self) -> bool {
        self.auto_attach.load(Ordering::Relaxed)
    }

    fn answer(&self, message: &Value, script: &mut Script) -> Result<Value, &'static str> {
        let info =
            |(id, url): &(String, String)| json!({"targetId": id, "type": "page", "url": url});
        let session = message["sessionId"].as_str().unwrap_or_default();
        let page = self.target_of(session);
        let method = message["method"].as_str().unwrap_or_default();
        if lock(&self.failing).iter().any(|failed| failed == method) {
            return Err("the fake fails this method");
        }
        if let Some((_, result)) = lock(&self.canned).iter().rev().find(|(m, _)| m == method) {
            return Ok(result.clone());
        }
        Ok(match method {
            "Target.getTargets" => {
                let infos: Vec<Value> = lock(&self.targets).iter().map(info).collect();
                json!({ "targetInfos": infos })
            }
            "Target.getTargetInfo" => {
                let Some(wanted) = message["params"]["targetId"].as_str().or(page.as_deref())
                else {
                    return Ok(json!({"targetInfo": {
                        "targetId": "BROWSER", "type": "browser", "title": "", "url": "",
                        "attached": true, "canAccessOpener": false,
                    }}));
                };
                let targets = lock(&self.targets);
                let found = targets.iter().find(|(id, _)| id == wanted);
                json!({ "targetInfo": info(found.ok_or("No target with given id found")?) })
            }
            "Target.attachToTarget" => {
                let target = message["params"]["targetId"].as_str();
                json!({ "sessionId": self.new_session("S", target) })
            }
            "Target.attachToBrowserTarget" => json!({ "sessionId": self.new_session("B", None) }),
            "Target.setAutoAttach" if self.playing() && session.starts_with('B') => {
                script.before = self.announce(session);
                json!({})
            }
            "Page.addScriptToEvaluateOnNewDocument" => {
                let n = lock(&self.sent)
                    .iter()
                    .filter(|sent| sent["method"] == "Page.addScriptToEvaluateOnNewDocument")
                    .count();
                json!({ "identifier": n.to_string() })
            }
            "Browser.getVersion" => json!({
                "protocolVersion": "1.3",
                "product": "Chrome/131.0.6778.86",
                "userAgent": FAKE_USER_AGENT,
            }),
            "Page.getFrameTree" if self.playing() => {
                let target = page.ok_or("Page.getFrameTree needs a page session")?;
                let url = self.url_of(&target).unwrap_or_default();
                json!({ "frameTree": { "frame": frame(&target, &url, &format!("L0-{target}")) } })
            }
            "Page.navigate" if self.playing() => {
                let target = page.ok_or("Page.navigate needs a page session")?;
                let url = message["params"]["url"].as_str().unwrap_or_default();
                let loader = format!("N{}", self.loaders.fetch_add(1, Ordering::Relaxed) + 1);
                if let Some(entry) = lock(&self.targets).iter_mut().find(|(id, _)| *id == target) {
                    entry.1 = url.to_owned();
                }
                script.after = committed(session, &target, url, &loader);
                json!({ "frameId": target, "loaderId": loader })
            }
            _ => json!({}),
        })
    }

    /// `Target.attachedToTarget` on the browser session `root` for every target.
    fn announce(&self, root: &str) -> Vec<Value> {
        let targets = lock(&self.targets).clone();
        targets
            .iter()
            .map(|(id, url)| {
                let session = self.new_session("A", Some(id));
                json!({
                    "method": "Target.attachedToTarget",
                    "sessionId": root,
                    "params": {
                        "sessionId": session,
                        "targetInfo": {
                            "targetId": id, "type": "page", "title": url, "url": url,
                            "attached": true, "canAccessOpener": false,
                            "browserContextId": "CONTEXT",
                        },
                        "waitingForDebugger": true,
                    },
                })
            })
            .collect()
    }
}

/// The main frame of `target` showing `url`; its id is the target's, as in Chromium.
fn frame(target: &str, url: &str, loader: &str) -> Value {
    let origin = url
        .split_once("://")
        .map(|(scheme, rest)| format!("{scheme}://{}", rest.split('/').next().unwrap_or_default()))
        .unwrap_or_default();
    json!({
        "id": target, "loaderId": loader, "url": url, "domainAndRegistry": "",
        "securityOrigin": origin, "mimeType": "text/html", "secureContextType": "Secure",
        "crossOriginIsolatedContextType": "NotIsolated", "gatedAPIFeatures": [],
    })
}

/// A navigation of `target` to `url` committed on `session`, then loaded.
fn committed(session: &str, target: &str, url: &str, loader: &str) -> Vec<Value> {
    let mut events = vec![json!({
        "method": "Page.frameNavigated",
        "sessionId": session,
        "params": { "frame": frame(target, url, loader), "type": "Navigation" },
    })];
    for name in ["DOMContentLoaded", "load"] {
        events.push(json!({
            "method": "Page.lifecycleEvent",
            "sessionId": session,
            "params": { "frameId": target, "loaderId": loader, "name": name, "timestamp": 1.0 },
        }));
    }
    events
}

impl CdpTransport for FakeBrowser {
    fn send(&self, frame: Vec<u8>) -> Result<(), EngineError> {
        let body = frame.strip_suffix(&[0]).unwrap_or(&frame);
        let message: Value = serde_json::from_slice(body)
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        lock(&self.sent).push(message.clone());
        let mut script = Script::default();
        let reply = match self.answer(&message, &mut script) {
            Ok(result) => json!({ "id": message["id"], "result": result }),
            Err(text) => {
                json!({ "id": message["id"], "error": { "code": -32602, "message": text } })
            }
        };
        let held = lock(&self.held).as_ref().is_some_and(|(method, session)| {
            message["method"] == method.as_str()
                && session
                    .as_ref()
                    .is_none_or(|session| message["sessionId"] == session.as_str())
        });
        let mut messages = script.before;
        messages.push(reply);
        messages.extend(script.after);
        if held {
            lock(&self.parked).push(messages);
            return Ok(());
        }
        for message in messages {
            self.emit(message);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "fake_tests.rs"]
mod tests;
