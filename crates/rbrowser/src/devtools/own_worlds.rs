//! rBrowser's own isolated worlds stay out of the docked DevTools (RB-090-51): the Console's
//! context picker lists the page's contexts only, not the sync agent's world, "Inspect
//! element"'s, the capture's, the reload's, the footer's or a plugin's `rbplugin-<id>`. The
//! relay ([`super::relay`]) passes neither a `Runtime.executionContextCreated` of such a world
//! nor its `executionContextDestroyed` to the frontend, so the frontend never knows the context
//! and cannot evaluate in it. The page's own contexts and an extension's content-script worlds
//! pass unchanged; the DevTools window and outer CDP clients see every world as before.

use std::collections::HashMap;

use serde_json::Value;

/// The sync agent's world (`rb_sync::event::WORLD_NAME`).
const AGENT_WORLD: &str = "rbrowser";
/// The prefix of the core's other worlds: `rbrowser-inspect`, `rbrowser-capture`,
/// `rbrowser-reload`, `rbrowser-footer`.
const CORE_PREFIX: &str = "rbrowser-";
/// The prefix of a plugin's world (`rb_plugin_host::world_name`).
const PLUGIN_PREFIX: &str = "rbplugin-";

/// Whether `name` is the name of one of rBrowser's isolated worlds.
pub fn is_own_world(name: &str) -> bool {
    name == AGENT_WORLD || name.starts_with(CORE_PREFIX) || name.starts_with(PLUGIN_PREFIX)
}

/// Whether an `ExecutionContextDescription` is a context of one of rBrowser's worlds: an
/// isolated world (`auxData.type`) of rBrowser's name.
fn is_own_context(context: &Value) -> bool {
    let isolated = context
        .get("auxData")
        .and_then(|aux| aux.get("type"))
        .and_then(Value::as_str)
        == Some("isolated");
    isolated
        && context
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(is_own_world)
}

/// One hidden context: its id in its session, and its unique id.
#[derive(Debug)]
struct Hidden {
    id: Option<i64>,
    unique: Option<String>,
}

/// The contexts of rBrowser's worlds the frontend was not told of, per session on the pipe (a
/// context id is unique in its target only).
#[derive(Debug, Default)]
pub struct OwnWorlds {
    hidden: HashMap<String, Vec<Hidden>>,
}

impl OwnWorlds {
    /// Whether the event `method` with `params` on `session` stays from the frontend: the
    /// creation of a context of rBrowser's worlds, and the end of one that was hidden.
    pub fn hides(&mut self, session: &str, method: &str, params: &Value) -> bool {
        match method {
            "Runtime.executionContextCreated" => {
                let Some(context) = params.get("context").filter(|c| is_own_context(c)) else {
                    return false;
                };
                self.hidden
                    .entry(session.to_owned())
                    .or_default()
                    .push(Hidden {
                        id: context.get("id").and_then(Value::as_i64),
                        unique: context
                            .get("uniqueId")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    });
                true
            }
            "Runtime.executionContextDestroyed" => self.ended(session, params),
            "Runtime.executionContextsCleared" => {
                self.forget(session);
                false
            }
            _ => false,
        }
    }

    /// The session is gone (a child detached): its contexts with it.
    pub fn forget(&mut self, session: &str) {
        self.hidden.remove(session);
    }

    /// Whether the context `params` names was hidden; it is forgotten.
    fn ended(&mut self, session: &str, params: &Value) -> bool {
        let id = params.get("executionContextId").and_then(Value::as_i64);
        let unique = params
            .get("executionContextUniqueId")
            .and_then(Value::as_str);
        let Some(contexts) = self.hidden.get_mut(session) else {
            return false;
        };
        let Some(index) = contexts.iter().position(|hidden| {
            (id.is_some() && hidden.id == id)
                || unique.is_some_and(|unique| hidden.unique.as_deref() == Some(unique))
        }) else {
            return false;
        };
        contexts.swap_remove(index);
        if contexts.is_empty() {
            self.hidden.remove(session);
        }
        true
    }
}

#[cfg(test)]
#[path = "own_worlds_tests.rs"]
mod tests;
