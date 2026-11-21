//! The sync agent on the host's side (RB-020-01, ADR 0001 Q7): the bundle of `agent/` embedded at
//! build time, injected into every view's page session - and every iframe's, through
//! auto-attach - as an isolated world, and the binding it reports through. The page never sees
//! the agent: its globals and the binding live in the world `rbrowser` only, and the page's CSP
//! does not apply there.
//!
//! Per CDP session: `Page.enable`, `Runtime.enable`, `Runtime.addBinding { executionContextName:
//! "rbrowser" }`, `Page.addScriptToEvaluateOnNewDocument { worldName: "rbrowser", runImmediately:
//! true }` with the boot configuration in front of the bundle, then `Target.setAutoAttach` for
//! iframes. [`SyncAgents`] follows the agents' contexts, turns `Runtime.bindingCalled` into
//! [`AgentReport`]s (parsed as untrusted input, `rb_sync::parse_message`), configures each agent
//! when it says ready and runs the core's commands in it. The SyncHub (RB-020-02) subscribes.
//! The protocol is `docs/sync-protocol.md`.
//!
//! The page API (RB-020-08) rides on the agents: [`PageApi`] registers `window.__rbrowser` in
//! every view's main world and hands each agent its view's state (`page.rs`, `page_state.rs`,
//! fed by `page_tasks.rs`), the bus relays the pages' messages (`bus.rs`, each view on its own
//! queue in `relay.rs`), the toggles are
//! `page_commands.rs` and the user-agent suffix `ua.rs`.
//!
//! The Framework DevTools (RB-080-24) ride on the same sessions: [`FrameworkHooks`] registers
//! the framework probe and the Vue and React DevTools backends in a view's main world
//! (`framework.rs`, fed by `framework_tasks.rs`); the agent relays the probe's report.

mod agents;
pub mod bus;
mod contexts;
pub mod framework;
pub mod framework_tasks;
pub mod page;
pub mod page_commands;
pub mod page_state;
pub mod page_tasks;
pub mod relay;
pub mod ua;

use rb_config::SyncConfig;
use rb_engine::EngineError;
use rb_sync::{
    AGENT_GLOBAL, AgentCommand, AgentConfig, AgentScrollMode, BINDING_NAME, BOOT_GLOBAL,
    CaptureSwitches, WORLD_NAME,
};
use serde_json::json;

use crate::cdp::CdpRouter;

pub use agents::{AgentReport, SyncAgents};
pub use framework::FrameworkHooks;
pub use page::PageApi;

/// The agent bundle (`agent/dist/rb-agent.js`), copied by `build.rs`; empty when `agent/` was not
/// built (`npm run build` there, part of `scripts/check.sh --preflight`).
pub const AGENT_SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/rb-agent.js"));

/// The agent's configuration for a view from the project's `sync.*` settings.
pub fn agent_config(sync: &SyncConfig, active: bool) -> AgentConfig {
    AgentConfig {
        active,
        capture: CaptureSwitches {
            click: sync.click,
            hover: sync.hover,
            scroll: sync.scroll,
            keyboard: sync.keyboard,
            input: sync.input,
            forms: sync.forms,
            focus: sync.focus,
        },
        hover_throttle_ms: sync.hover_throttle_ms,
        scroll_debounce_ms: sync.scroll_debounce_ms,
        scroll_mode: scroll_mode_of(sync.scroll_mode),
        record: None,
    }
}

/// `mode` as the agent knows it: in anchor mode it listens for no wheel (RB-090-33).
pub fn scroll_mode_of(mode: rb_config::ScrollMode) -> AgentScrollMode {
    match mode {
        rb_config::ScrollMode::Anchor => AgentScrollMode::Anchor,
        rb_config::ScrollMode::Wheel => AgentScrollMode::Wheel,
    }
}

/// The script injected into the world: the boot configuration, then `bundle`. `None` for an
/// empty bundle (no agent built).
pub fn boot_source(bundle: &str, config: &AgentConfig) -> Option<String> {
    if bundle.trim().is_empty() {
        return None;
    }
    let config = serde_json::to_string(config).ok()?;
    Some(format!(
        "\"use strict\";globalThis.{BOOT_GLOBAL}={config};\n{bundle}"
    ))
}

/// The expression that runs `command` in the agent's world. JSON is a JavaScript expression, so
/// nothing in the command can break out of the call.
pub fn command_expression(command: &AgentCommand) -> Result<String, EngineError> {
    let json =
        serde_json::to_string(command).map_err(|error| EngineError::Invalid(error.to_string()))?;
    Ok(format!("globalThis.{AGENT_GLOBAL}.command({json})"))
}

/// Injects `source` into the target of `session` (and, through auto-attach, its iframes).
pub(crate) async fn inject(
    cdp: &CdpRouter,
    session: &str,
    source: &str,
) -> Result<(), EngineError> {
    cdp.call("Page.enable", json!({}), Some(session)).await?;
    cdp.call("Runtime.enable", json!({}), Some(session)).await?;
    cdp.call(
        "Runtime.addBinding",
        json!({ "name": BINDING_NAME, "executionContextName": WORLD_NAME }),
        Some(session),
    )
    .await?;
    cdp.call(
        "Page.addScriptToEvaluateOnNewDocument",
        json!({ "source": source, "worldName": WORLD_NAME, "runImmediately": true }),
        Some(session),
    )
    .await?;
    // Out-of-process iframes are targets of their own; the script above covers the others.
    cdp.call(
        "Target.setAutoAttach",
        json!({
            "autoAttach": true,
            "waitForDebuggerOnStart": false,
            "flatten": true,
            "filter": [{ "type": "iframe" }],
        }),
        Some(session),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
