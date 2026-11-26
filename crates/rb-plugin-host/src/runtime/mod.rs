//! Wasmtime component runtime with deny-by-default imports: the store, its limits and the
//! deadline of each call.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host/src/runtime.rs` (itself from
//! rDownloader's): the same store limits (memory, instances, tables, table elements), fuel, the
//! epoch deadline consulted against a wall-clock instant, host time credited back, and an epoch
//! callback that yields, so a busy plugin hands the tokio worker back every tick. Not taken:
//! rCommander's cancellation flag and HTTP time budget - a call here is bounded by its fuel, its
//! deadline and the caller's wall clock - and its long-lived stores: a store lives for one call.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use rb_plugin_types::{Failure, FailureKind, codes};
use wasmtime::{
    Engine, Store, StoreLimits, StoreLimitsBuilder, UpdateDeadline, component::Component,
};

use crate::{
    PluginLimits, context::PluginContext, engine::SharedEngine, failure, log_budget::LogBudget,
    manifest::PluginManifest,
};

mod imports;

pub(crate) use imports::{allowed_imports, refused_import};

pub(crate) const EPOCH_TICK: Duration = Duration::from_millis(10);
/// Most elements one plugin table may hold (rDownloader's PL-01): table storage is not counted
/// against `memory_bytes`, so without this cap a `table.grow` loop took host memory at will.
pub(crate) const MAX_TABLE_ELEMENTS: usize = 10_000;

/// The state of one store, reached only through the host functions linked for its grants.
pub(crate) struct PluginStoreState {
    limits: StoreLimits,
    /// Wall-clock instant at which the running call's compute budget runs out.
    execution_deadline: Instant,
    /// The lines the running call may still log.
    pub(crate) log_budget: LogBudget,
    /// The plugin's host functions behind its grants.
    pub(crate) context: Arc<PluginContext>,
    /// The `out-stream` file of this call not yet committed (RB-099-24); dropped with the store,
    /// which removes it, however the call ends.
    pub(crate) out_stream: Option<crate::context::large::Staged>,
    /// The epoch moves as long as the store lives, one call (RB-090-38).
    _ticking: crate::engine::Ticking,
}

impl PluginStoreState {
    /// Credits time spent inside a host call back to the deadline: a view evaluating or a server
    /// answering is not the plugin thinking (rDownloader's RA-HOST-05). Fuel still runs out on a
    /// spinning one.
    pub(crate) fn credit_host_time(&mut self, elapsed: Duration) {
        self.execution_deadline += elapsed;
    }
}

/// Compiles components and creates resource-limited stores. Cheap to clone: the engine behind
/// it is the process's one shared engine ([`crate::engine`]); what is per plugin here is the
/// limits its stores are given.
#[derive(Clone)]
pub(crate) struct SandboxEngine {
    shared: Arc<SharedEngine>,
    limits: PluginLimits,
    timeout: Duration,
}

impl SandboxEngine {
    /// A sandbox on the shared engine with the manifest's limits.
    pub(crate) fn new(limits: PluginLimits) -> Result<Self> {
        limits.check().map_err(anyhow::Error::msg)?;
        Ok(Self {
            shared: crate::engine::shared()?,
            timeout: Duration::from_millis(limits.timeout()),
            limits,
        })
    }

    /// The compute time of one call.
    pub(crate) fn timeout(&self) -> Duration {
        self.timeout
    }

    /// How many calls of the plugin may run at once (`limits.max_concurrent`).
    pub(crate) fn max_concurrent(&self) -> u32 {
        self.limits.max_concurrent
    }

    /// Compiles one component and refuses every import the manifest does not grant.
    ///
    /// The first of the two places a grant is enforced - the linker is the second. A package
    /// that reaches beyond its manifest is refused where its author sees it, before any call:
    /// `plugins.error.capability_denied` naming the capability when the import is one a
    /// capability would grant, `plugins.error.denied` naming the import otherwise (WASI, another
    /// package, another contract version).
    pub(crate) fn compile_component(
        &self,
        bytes: &[u8],
        manifest: &PluginManifest,
    ) -> Result<Component, Failure> {
        // Compiled once per content; the grant is checked every time, because the same bytes
        // may arrive under a different manifest.
        let component = self.shared.compile(bytes).map_err(|error| {
            tracing::debug!(%error, "a plugin component does not compile");
            failure::coded(FailureKind::BadData, codes::INVALID)
                .with_param("reason", "the component does not compile")
        })?;
        let allowed = allowed_imports(manifest);
        for (name, _) in component.component_type().imports(self.engine()) {
            if !allowed.iter().any(|allowed| allowed == name) {
                return Err(refused_import(name));
            }
        }
        Ok(component)
    }

    /// A fresh store for one call of the plugin behind `context`.
    pub(crate) fn create_store(
        &self,
        context: Arc<PluginContext>,
    ) -> Result<Store<PluginStoreState>> {
        let memory_bytes = usize::try_from(self.limits.memory_bytes)
            .context("plugin memory limit exceeds platform usize")?;
        let state = PluginStoreState {
            limits: StoreLimitsBuilder::new()
                .memory_size(memory_bytes)
                .instances(32)
                .tables(8)
                .table_elements(MAX_TABLE_ELEMENTS)
                .memories(1)
                .trap_on_grow_failure(true)
                .build(),
            execution_deadline: Instant::now() + self.timeout,
            log_budget: LogBudget::new(context.plugin_id.clone(), context.version.clone()),
            context,
            out_stream: None,
            _ticking: self.shared.ticking(),
        };
        let mut store = Store::new(self.engine(), state);
        store.limiter(|state| &mut state.limits);
        store.epoch_deadline_callback(|context| {
            if Instant::now() >= context.data().execution_deadline {
                return Ok(UpdateDeadline::Interrupt);
            }
            Ok(UpdateDeadline::Yield(1))
        });
        Ok(store)
    }

    /// Starts one call of `operation` in `store`: its fuel, its deadline, its log budget.
    pub(crate) fn begin_call(
        &self,
        store: &mut Store<PluginStoreState>,
        operation: &'static str,
    ) -> Result<()> {
        store
            .set_fuel(self.limits.fuel)
            .map_err(|error| anyhow::anyhow!("configure plugin fuel: {error}"))?;
        store.set_epoch_deadline(1);
        let state = store.data_mut();
        state.execution_deadline = Instant::now() + self.timeout;
        state.log_budget.begin(operation);
        Ok(())
    }

    /// The underlying engine, for the linkers.
    pub(crate) fn engine(&self) -> &Engine {
        self.shared.engine()
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
