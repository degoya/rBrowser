//! A plugin call with the consent probe (RB-094-09): the runtime's `call-tool` and
//! `run-command` go through [`with_probe`]. When the arguments offer the probe (only a plugin
//! granted `consent-probe` gets the offer, `super::args::offered`) and the plugin's answer asks
//! for one, the host runs it ([`Prober`]) and calls the same tool or command once more with the
//! capture as `$consent`; that second answer is the call's. One probe per call: a plugin that
//! asks again fails the call. A plugin not offered the probe gets its answer handed on as it is.

use std::sync::Arc;

use async_trait::async_trait;
use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::Value;

use super::args::{ProbeRequest, is_offered, requested, with_consent};
use crate::plugins::runtime::Component;

/// What a call runs.
#[derive(Clone, Copy, Debug)]
pub enum Call<'a> {
    /// A tool of the plugin's manifest.
    Tool(&'a str),
    /// A command of the plugin's manifest.
    Command(&'a str),
}

/// Runs a probe: the window's (`super::prober::HostProber`) in the app, a fake in tests.
#[async_trait]
pub trait Prober: Send + Sync {
    /// The capture of `request`, the `$consent` argument.
    async fn probe(&self, request: &ProbeRequest) -> Result<Value, Failure>;
}

async fn invoke(
    component: &dyn Component,
    call: Call<'_>,
    args: &Value,
) -> Result<String, Failure> {
    let args = args.to_string();
    match call {
        Call::Tool(tool) => component.call_tool(tool, &args).await,
        Call::Command(command) => component.run_command(command, &args).await,
    }
}

/// Runs `call` of `component` with `args` (as the runtime fed them); with the probe when they
/// offer it and the plugin asks, `prober` running it.
///
/// # Errors
///
/// The plugin's failure; `plugins.error.invalid` for a request without a valid address or
/// collector; `plugins.error.not_available` without a window to probe in; the probe's failure;
/// `plugins.error.bad_output` for a plugin that asks twice.
pub async fn with_probe(
    component: &dyn Component,
    call: Call<'_>,
    args: &Value,
    prober: Option<Arc<dyn Prober>>,
) -> Result<String, Failure> {
    let raw = invoke(component, call, args).await?;
    if !is_offered(args) {
        return Ok(raw);
    }
    let Some(request) = requested(&raw) else {
        return Ok(raw);
    };
    let request = request?;
    let prober = prober.ok_or_else(|| {
        Failure::coded(FailureKind::NotSupported, codes::NOT_AVAILABLE)
            .with_param("service", "consent probe")
    })?;
    let consent = prober.probe(&request).await?;
    let raw = invoke(component, call, &with_consent(args, consent)).await?;
    if requested(&raw).is_some() {
        return Err(Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT));
    }
    Ok(raw)
}

#[cfg(test)]
#[path = "call_tests.rs"]
mod tests;
