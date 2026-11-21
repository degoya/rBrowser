//! The sync agent in a Safari view. Classic WebDriver has no preload script and no isolated
//! world, so the agent boots on demand through `execute/sync` in the page's main world - inside a
//! private scope as in WebKit (`sidecar/src/webkit/agent.ts`), its command entry under a random,
//! non-enumerable name ([`random_name`]) - and each command first finds out whether the document
//! has it ([`command`]): a new document boots it again. A Safari view never leads (Safari locks
//! input in automated windows), so the agent's emitter does nothing and its configuration is a
//! follower's. What the page can see is what it can see in WebKit: one non-enumerable global.
//! Every value goes in as a WebDriver argument or a JSON literal, so nothing breaks out.
//!
//! **Boundary (RB-095-05):** that global is findable from the page's main world
//! (`Object.getOwnPropertyNames(globalThis)` lists it). A page can call it - no more in its own
//! document than its own script can - or replace it and so break this view's replay; it cannot
//! report, as the view only follows (`docs/security/threat-model.md`, residual risks).

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};

use rb_sync::{
    AGENT_GLOBAL, AgentCommand, AgentConfig, AgentScrollMode, BINDING_NAME, BOOT_GLOBAL,
    CaptureSwitches, CommandResult, DEFAULT_SCROLL_DEBOUNCE_MS,
};
use serde_json::Value;

use crate::client::Session;
use crate::error::SafariError;

/// Runs `arguments[1]` in the agent named `arguments[0]`; `null` when the document has none.
pub const COMMAND_SCRIPT: &str = "\"use strict\";\nconst handle = globalThis[arguments[0]];\nif (handle === undefined || typeof handle.command !== \"function\") { return null; }\nreturn JSON.stringify(handle.command(arguments[1]));";

/// No form submits for `arguments[0]` ms (the leader submits; the view follows by URL).
pub const BLOCK_SUBMIT: &str = "const ms = arguments[0];\nconst stop = (e) => { e.preventDefault(); e.stopImmediatePropagation(); };\nwindow.addEventListener('submit', stop, true);\nsetTimeout(() => window.removeEventListener('submit', stop, true), ms);\nreturn 'ok';";

/// A follower's configuration: the agent captures nothing (the app sends the project's sync
/// switches with `active: false` in its place).
pub fn follower_config() -> AgentConfig {
    AgentConfig {
        active: false,
        capture: CaptureSwitches {
            click: false,
            hover: false,
            scroll: false,
            keyboard: false,
            input: false,
            forms: false,
            focus: false,
        },
        hover_throttle_ms: 100,
        scroll_debounce_ms: DEFAULT_SCROLL_DEBOUNCE_MS,
        scroll_mode: AgentScrollMode::Anchor,
        record: None,
    }
}

/// A name nobody guesses for the agent's command entry: `__rb` and 32 hex digits.
pub fn random_name() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let half = || RandomState::new().hash_one(NEXT.fetch_add(1, Ordering::Relaxed));
    format!("__rb{:016x}{:016x}", half(), half())
}

/// The script that boots `bundle` with `config` under `name`, once per document; `None`
/// without a bundle.
pub fn boot_script(bundle: &str, config: &AgentConfig, name: &str) -> Option<String> {
    if bundle.trim().is_empty() {
        return None;
    }
    let config = serde_json::to_string(config).ok()?;
    let name = serde_json::to_string(name).ok()?;
    Some(format!(
        "\"use strict\";\nconst name = {name};\nif (Object.prototype.hasOwnProperty.call(globalThis, name)) {{ return true; }}\nconst scope = {{ {BINDING_NAME}: () => {{}}, {BOOT_GLOBAL}: {config} }};\n(function (globalThis) {{\n{bundle}\n}}).call(undefined, scope);\nconst handle = scope.{AGENT_GLOBAL};\nif (handle === undefined) {{ return false; }}\nObject.defineProperty(globalThis, name, {{ value: handle, enumerable: false, configurable: false, writable: false }});\nreturn true;"
    ))
}

/// The agent of one Safari view: its name and, when there is a bundle, the boot script.
#[derive(Clone, Debug)]
pub struct Agent {
    /// The command entry's name.
    pub name: String,
    /// The boot script; `None` without a bundle (tests).
    pub boot: Option<std::sync::Arc<str>>,
}

/// Runs `command` in the agent of `session`'s document, booting it first when the document has
/// none. The answer is untrusted: anything but a command result is refused.
pub async fn command(
    session: &Session,
    agent: &Agent,
    command: &AgentCommand,
) -> Result<CommandResult, SafariError> {
    let json = serde_json::to_value(command).map_err(|_| protocol("the command is not JSON"))?;
    for attempt in 0..2 {
        let answer = session
            .execute(
                COMMAND_SCRIPT,
                vec![Value::from(agent.name.clone()), json.clone()],
            )
            .await?;
        if let Some(text) = answer.as_str() {
            return serde_json::from_str(text)
                .map_err(|_| protocol("the agent's answer is not a command result"));
        }
        match (&agent.boot, attempt) {
            (Some(boot), 0) => {
                session.execute(boot, Vec::new()).await?;
            }
            _ => break,
        }
    }
    Err(protocol("the page has no sync agent"))
}

fn protocol(reason: &str) -> SafariError {
    SafariError::Protocol {
        command: "execute script".to_owned(),
        reason: reason.to_owned(),
    }
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
