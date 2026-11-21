//! A plugin call with the load test's work (RB-096-07): the runtime's `call-tool` and
//! `run-command` go through [`with_load`], around the API client's
//! (`crate::api_client::call::with_api`, which wraps the consent probe's). When the arguments
//! offer the load test (only a plugin granted `load-test` gets the offer,
//! [`super::protocol::offered`]) and the plugin's answer asks for an op the origin may run, the
//! window's [`LoadHost`] runs it and the same tool or command is called once more with the op's
//! answer as `$loadResult`; that second answer is the call's. One op per call: a plugin that
//! asks again fails the call. An op's failure is the call's.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::Value;

use super::protocol::{Op, Origin, offer_of, permitted, requested, with_result};
use crate::api_client::call::with_api;
use crate::plugins::consent::Call;
use crate::plugins::runtime::Component;

/// Runs an op: the window's ([`super::window::WindowLoad`]) in the app, a fake in tests.
#[async_trait]
pub trait LoadHost: Send + Sync {
    /// The answer of `op` for a caller `origin` (already [`permitted`]).
    async fn run(&self, op: Op, origin: Origin) -> Result<Value, Failure>;
}

static HOST: Mutex<Option<Arc<dyn LoadHost>>> = Mutex::new(None);

/// The window's host from now on.
pub fn set_host(host: Arc<dyn LoadHost>) {
    *rb_util::lock(&HOST) = Some(host);
}

/// The running window's host; none before [`set_host`].
#[must_use]
pub fn host() -> Option<Arc<dyn LoadHost>> {
    rb_util::lock(&HOST).clone()
}

/// Runs `call` of `component` with `args` (as the runtime fed them), the consent probe, the
/// API client's op and the load test's op included.
///
/// # Errors
///
/// As [`with_host`].
pub async fn with_load(
    component: &dyn Component,
    call: Call<'_>,
    args: &Value,
) -> Result<String, Failure> {
    with_host(component, call, args, host()).await
}

/// [`with_load`] with `host` running the op.
///
/// # Errors
///
/// The inner call's failure; `plugins.error.invalid` for an op that does not read;
/// `plugins.error.capability_denied` for an op the origin may not run;
/// `plugins.error.not_available` without a window; the op's failure;
/// `plugins.error.bad_output` for a plugin that asks twice.
pub async fn with_host(
    component: &dyn Component,
    call: Call<'_>,
    args: &Value,
    host: Option<Arc<dyn LoadHost>>,
) -> Result<String, Failure> {
    let raw = with_api(component, call, args).await?;
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
            .with_param("service", "load test")
    })?;
    let result = host.run(op, origin).await?;
    let again = with_result(args, result);
    let raw = with_api(component, call, &again).await?;
    if requested(&raw).is_some() {
        return Err(Failure::coded(FailureKind::BadData, codes::BAD_OUTPUT));
    }
    Ok(raw)
}

#[cfg(test)]
#[path = "call_tests.rs"]
mod tests;
