//! Framework DevTools (RB-080-24, `docs/plugins.md`): the Vue and React DevTools as panels of
//! rBrowser, for the active view.
//!
//! - The **backends** are the core's: `react-devtools-inline` and `@vue/devtools-kit` as
//!   main-world scripts (`agent/src/framework/`), speaking over DOM events of the page.
//! - The **page script** (`page/bridge.js`, built from `web/`) relays those events to the panels
//!   over the plugin bridge (`rbPlugin.emit`, chunked under the message limit) and the panels'
//!   messages back (`rbPlugin.on`).
//! - The **panels** (`panel/`, their scripts in `panel/dist/`, built from `web/`) are the React
//!   DevTools frontend and a Vue panel over the kit's RPC: the component tree, the custom
//!   inspectors (Pinia, the router's routes), the current route and the timeline (RB-080-31).
//! - The **component** here routes: the command `vue` / `react` opens the panel, the tool
//!   `active_view` names the view the panels follow, the tool `send` posts a panel's envelope to
//!   a view's page script.
//!
//! The logic is plain JSON against the native [`PluginHost`]; `guest.rs` translates it into the
//! contract's vocabulary and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

use rb_plugin_api::feature::Feature;
use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde_json::{Value, json};

/// The plugin's id: the prefix of its keys.
pub const PLUGIN_ID: &str = "framework-devtools";
/// The frameworks: the command, panel and envelope names.
pub const FRAMEWORKS: [&str; 2] = ["vue", "react"];
/// The tool naming the view the panels follow.
pub const ACTIVE_VIEW: &str = "active_view";
/// The tool posting a panel's envelope to a view.
pub const SEND: &str = "send";
/// The largest envelope the component posts, in bytes of its JSON text (under the host's
/// 64 KiB message limit, with the post's own envelope).
pub const MAX_ENVELOPE_BYTES: usize = 60 * 1024;

fn unknown(name: &str) -> Failure {
    Failure::coded(FailureKind::NotFound, codes::UNKNOWN_CONTRIBUTION)
        .with_param("name", name.chars().take(64).collect::<String>())
}

fn invalid(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

/// `args` as a JSON object; empty text is `{}` (a shortcut's call).
fn object(args: &str) -> Result<Value, Failure> {
    if args.trim().is_empty() {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_str(args).map_err(|_| invalid("args"))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(invalid("args"))
    }
}

/// The command `vue` or `react`: opens the framework's panel.
pub fn open_panel(framework: &str) -> Result<String, Failure> {
    if !FRAMEWORKS.contains(&framework) {
        return Err(unknown(framework));
    }
    Ok(json!({ "$open": { "panel": framework } }).to_string())
}

/// `active_view`: `{"view": <id>}`, `null` without an active view.
pub fn active_view(host: &mut dyn PluginHost) -> Result<String, Failure> {
    let view = host.active_view()?;
    Ok(json!({ "view": view }).to_string())
}

/// `send`: posts `{"framework", "envelope"}` to `view`'s page script, which hands the envelope
/// to the framework's backend. The envelope is the panel's: `{"t": "connect"}`,
/// `{"t": "disconnect"}` or `{"t": "m", "d": ...}` of the framework's own protocol.
pub fn send(args: &str) -> Result<String, Failure> {
    let args = object(args)?;
    let view = args["view"]
        .as_u64()
        .and_then(|id| ViewId::try_from(id).ok())
        .ok_or_else(|| invalid("view"))?;
    let framework = args["framework"]
        .as_str()
        .filter(|name| FRAMEWORKS.contains(name))
        .ok_or_else(|| invalid("framework"))?;
    let envelope = &args["envelope"];
    let kind = envelope["t"].as_str().ok_or_else(|| invalid("envelope"))?;
    match kind {
        "connect" | "disconnect" => {}
        "m" if !envelope["d"].is_null() => {}
        _ => return Err(invalid("envelope")),
    }
    if envelope.to_string().len() > MAX_ENVELOPE_BYTES {
        return Err(Failure::coded(FailureKind::Limit, codes::TOO_LARGE));
    }
    Ok(json!({
        "$post": [{
            "target": view,
            "message": { "framework": framework, "envelope": envelope },
        }],
    })
    .to_string())
}

/// The feature plugin.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameworkDevtools;

impl Feature for FrameworkDevtools {
    fn run_command(
        &mut self,
        _host: &mut dyn PluginHost,
        command: &str,
        _args: &str,
    ) -> Result<String, Failure> {
        open_panel(command)
    }

    fn call_tool(
        &mut self,
        host: &mut dyn PluginHost,
        tool: &str,
        args: &str,
    ) -> Result<String, Failure> {
        match tool {
            ACTIVE_VIEW => active_view(host),
            SEND => send(args),
            other => Err(unknown(other)),
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
