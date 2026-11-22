//! The sandbox: one engine, the engine component compiled and linked once, a fresh store and
//! instance per run. A run is bounded by its memory (`StoreLimits`) and its time: the epoch
//! callback yields every tick and interrupts past the deadline, and the whole run - the tool
//! calls included - sits in a tokio timeout of the same length.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use wasmtime::component::{Component, ComponentExportIndex, InstancePre, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder, UpdateDeadline};

use crate::ticker::{Ticker, Ticking};
use crate::{ENGINE_INTERFACE, HOST_INTERFACE, Tools};

/// The native stack wasm code may use; Boa's parser and interpreter recurse.
const MAX_WASM_STACK: usize = 2 << 20;
/// The stack of the fiber a run executes on; larger than [`MAX_WASM_STACK`].
const ASYNC_STACK: usize = 4 << 20;
/// Most elements one table may hold: the engine's function table is fixed at build time.
const MAX_TABLE_ELEMENTS: usize = 200_000;

/// The limits of one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The whole run, its tool calls included.
    pub timeout: Duration,
    /// The engine instance's linear memory.
    pub memory_bytes: usize,
}

/// Why a run gave no answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunError {
    /// The script failed; the engine's failure as JSON text (`{"code", "name", "message", ...}`).
    Script(String),
    /// The time ran out.
    Timeout,
    /// The memory limit was reached.
    OutOfMemory,
    /// The engine trapped or could not run; one line, no backtrace.
    Engine(String),
}

/// The state of one run's store.
struct RunState {
    limits: StoreLimits,
    deadline: Instant,
    tools: Arc<dyn Tools>,
    _ticking: Ticking,
}

/// The script engine, ready to run.
pub struct Sandbox {
    engine: Engine,
    pre: InstancePre<RunState>,
    run: ComponentExportIndex,
    ticker: Ticker,
}

impl Sandbox {
    /// The sandbox over the engine component rBrowser ships ([`crate::ENGINE`]).
    ///
    /// # Errors
    ///
    /// The component does not compile, imports more than the host interface, or lacks `run`.
    pub fn new() -> anyhow::Result<Self> {
        Self::from_component(crate::ENGINE)
    }

    /// The sandbox over the component `bytes` (the tests' small ones).
    ///
    /// # Errors
    ///
    /// As [`Sandbox::new`].
    pub fn from_component(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut config = Config::new();
        config
            .wasm_component_model(true)
            .epoch_interruption(true)
            .max_wasm_stack(MAX_WASM_STACK)
            .async_stack_size(ASYNC_STACK);
        let engine = Engine::new(&config)
            .map_err(|error| anyhow::anyhow!("create the script engine: {error}"))?;
        let component = Component::from_binary(&engine, bytes)
            .map_err(|error| anyhow::anyhow!("compile the script engine component: {error}"))?;
        for (name, _) in component.component_type().imports(&engine) {
            anyhow::ensure!(
                name == HOST_INTERFACE,
                "the script engine component imports `{name}`; only {HOST_INTERFACE} is linked"
            );
        }
        let interface = component
            .get_export_index(None, ENGINE_INTERFACE)
            .context("the script engine component exports no engine interface")?;
        let run = component
            .get_export_index(Some(&interface), "run")
            .context("the script engine component exports no run")?;
        let mut linker = Linker::new(&engine);
        link(&mut linker).map_err(|error| anyhow::anyhow!("link the host: {error}"))?;
        let pre = linker
            .instantiate_pre(&component)
            .map_err(|error| anyhow::anyhow!("the script engine does not link: {error}"))?;
        let ticker = Ticker::new(engine.clone());
        Ok(Self {
            engine,
            pre,
            run,
            ticker,
        })
    }

    /// Runs `code` with `args` and `target` (JSON texts) in a fresh instance; its tool calls go
    /// to `tools`. Ok: the engine's answer as JSON text (`{"value", "logs"}`).
    ///
    /// # Errors
    ///
    /// The script failed, the time or the memory ran out, or the engine trapped.
    pub async fn run(
        &self,
        code: &str,
        args: &str,
        target: &str,
        limits: Limits,
        tools: Arc<dyn Tools>,
    ) -> Result<String, RunError> {
        let state = RunState {
            limits: StoreLimitsBuilder::new()
                .memory_size(limits.memory_bytes)
                .memories(1)
                .instances(16)
                .tables(16)
                .table_elements(MAX_TABLE_ELEMENTS)
                .trap_on_grow_failure(true)
                .build(),
            deadline: Instant::now() + limits.timeout,
            tools,
            _ticking: self.ticker.ticking(),
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store.epoch_deadline_callback(|context| {
            if Instant::now() >= context.data().deadline {
                return Ok(UpdateDeadline::Interrupt);
            }
            Ok(UpdateDeadline::Yield(1))
        });
        store.set_epoch_deadline(1);
        let params = (code.to_owned(), args.to_owned(), target.to_owned());
        let call = async {
            let instance = self.pre.instantiate_async(&mut store).await?;
            let function = instance
                .get_typed_func::<(String, String, String), (Result<String, String>,)>(
                    &mut store, self.run,
                )?;
            function.call_async(&mut store, params).await
        };
        match tokio::time::timeout(limits.timeout, call).await {
            Ok(Ok((Ok(answer),))) => Ok(answer),
            Ok(Ok((Err(failure),))) => Err(RunError::Script(failure)),
            Ok(Err(error)) => Err(stopped(&error)),
            Err(_) => Err(RunError::Timeout),
        }
    }
}

/// `rbrowser:script/host`: the tool call and the clock, nothing else.
fn link(linker: &mut Linker<RunState>) -> wasmtime::Result<()> {
    let mut host = linker.instance(HOST_INTERFACE)?;
    host.func_wrap_async::<(String, String), (Result<String, String>,), _>(
        "call",
        |store, (tool, arguments)| {
            let tools = Arc::clone(&store.data().tools);
            Box::new(async move { Ok((tools.call(tool, arguments).await,)) })
        },
    )?;
    host.func_wrap_async::<(), (u64,), _>("clock-ms", |_store, ()| {
        Box::new(async move {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
                .unwrap_or_default();
            Ok((now,))
        })
    })?;
    Ok(())
}

/// The failure of a script that ran out of stack (deep recursion), as the engine words one.
const STACK_FAILURE: &str = r#"{"code":"script.stack","name":"RangeError","message":"the script ran out of stack (too deep a recursion)"}"#;
/// The failure of a script that stopped the engine (a panic inside it).
const CRASH_FAILURE: &str = r#"{"code":"script.error","name":"InternalError","message":"the script stopped the engine (a panic inside it)"}"#;

/// A trap or a refused growth as a [`RunError`]: the interrupt is the deadline, a refused
/// memory growth (an error of the limiter, not a `Trap`) the memory limit; a stack overflow and
/// a panic are what the script did, not a broken engine.
fn stopped(error: &wasmtime::Error) -> RunError {
    if let Some(trap) = error.downcast_ref::<wasmtime::Trap>() {
        return match trap {
            wasmtime::Trap::Interrupt => RunError::Timeout,
            wasmtime::Trap::StackOverflow => RunError::Script(STACK_FAILURE.to_owned()),
            wasmtime::Trap::UnreachableCodeReached => RunError::Script(CRASH_FAILURE.to_owned()),
            other => RunError::Engine(other.to_string()),
        };
    }
    let reason = error.root_cause().to_string();
    if reason.to_ascii_lowercase().contains("memory") {
        RunError::OutOfMemory
    } else {
        RunError::Engine(reason.lines().next().unwrap_or_default().to_owned())
    }
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod tests;
