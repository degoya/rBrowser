//! A plugin call with the API client's work (RB-096-01): the runtime's `call-tool` and
//! `run-command` go through [`with_api`], around the consent probe's
//! (`crate::plugins::consent::call::with_probe`). When the arguments offer the API client (only
//! a plugin granted `api-client` gets the offer, [`super::protocol::offered`]) and the plugin's
//! answer asks for an op the origin may run, the window's [`ApiHost`] runs it and the same tool
//! or command is called once more with the op's answer as `$apiResult`; that second answer is
//! the call's. One op per call: a plugin that asks again fails the call. An op's failure is the
//! call's, so the panel hears the code and its parameters (`host_not_allowed` with the host).

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::Value;

use super::protocol::{Op, Origin, offer_of, permitted, requested, with_result};
use crate::cdp::lock;
use crate::plugins::consent::{self, Call};
use crate::plugins::runtime::Component;

/// Runs an op: the window's ([`super::window::WindowApi`]) in the app, a fake in tests.
#[async_trait]
pub trait ApiHost: Send + Sync {
    /// The answer of `op` for a caller `origin` (already [`permitted`]).
    async fn run(&self, op: Op, origin: Origin) -> Result<Value, Failure>;
}

static HOST: Mutex<Option<Arc<dyn ApiHost>>> = Mutex::new(None);

/// The window's host from now on.
pub fn set_host(host: Arc<dyn ApiHost>) {
    *lock(&HOST) = Some(host);
}

/// The running window's host; none before [`set_host`].
#[must_use]
pub fn host() -> Option<Arc<dyn ApiHost>> {
    lock(&HOST).clone()
}

/// Runs `call` of `component` with `args` (as the runtime fed them), the consent probe and the
/// API client's op included, the window's host running the op.
///
/// # Errors
///
/// As [`with_host`].
pub async fn with_api(
    component: &dyn Component,
    call: Call<'_>,
    args: &Value,
) -> Result<String, Failure> {
    with_host(component, call, args, host()).await
}

/// [`with_api`] with `host` running the op.
///
/// # Errors
///
/// The plugin's or the probe's failure; `plugins.error.invalid` for an op that does not read;
/// `plugins.error.capability_denied` for an op the origin may not run;
/// `plugins.error.not_available` without a window; the op's failure;
/// `plugins.error.bad_output` for a plugin that asks twice.
pub async fn with_host(
    component: &dyn Component,
    call: Call<'_>,
    args: &Value,
    host: Option<Arc<dyn ApiHost>>,
) -> Result<String, Failure> {
    let raw = consent::call::with_probe(component, call, args, consent::prober()).await?;
    let Some(origin) = offer_of(args) else {
        return Ok(raw);
    };
    let Some(op) = requested(&raw) else {
        return Ok(raw);
    };
    let op = op?;
    permitted(&op, origin)?;
    let host = host.ok_or_else(|| {
        Failure::coded(FailureKind::NotSupported, codes::NOT_AVAILABLE)
            .with_param("service", "api client")
    })?;
    let result = host.run(op, origin).await?;
    let again = with_result(args, result);
    let raw = consent::call::with_probe(component, call, &again, consent::prober()).await?;
    if requested(&raw).is_some() {
        return Err(Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT));
    }
    Ok(raw)
}

#[cfg(test)]
#[path = "call_tests.rs"]
mod tests;
