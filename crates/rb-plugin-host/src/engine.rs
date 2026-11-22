//! The one Wasmtime engine every plugin in this process is compiled and run on.
//!
//! Copied from rCommander's `crates/rc-plugin-host/src/engine.rs` (itself from rDownloader's,
//! RD-130-06), unchanged in substance: the engine configuration never differs between plugins -
//! the limits belong to a store, not to an engine - so one engine serves them all, one ticker
//! drives every epoch deadline, and a component is compiled once per exact byte content and
//! handed out again from memory. The ticker advances the epoch only while a call's store lives
//! ([`SharedEngine::ticking`]); idle, its thread is parked and does not wake (RB-090-38). What survives a restart is [`crate::compile_cache`]'s business.
//!
//! Whether that cache is used is the setting `plugins.compileCache` ([`set_compile_cache`],
//! RB-094-30): the cache is part of an engine's configuration, so a changed choice builds a new
//! engine for the next load; the plugins loaded before keep the engine they were compiled on.
//!
//! Handing a compiled component out again bypasses nothing: it is looked up by the SHA-256 of the
//! very bytes the caller passes, which the caller has verified before asking.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
};

use anyhow::Result;
use sha2::{Digest, Sha256};
use wasmtime::{Config, Engine, component::Component};

use crate::compile_cache::CompileCache;
use crate::recover::LogPanic as _;

/// Distinct component contents kept compiled in memory; past this the map starts over.
const MAX_COMPILED: usize = 256;

static SHARED: EngineSlot = EngineSlot::new();

/// Keeps compiled plugin code under `directory` from the next plugin load on, so a restart with
/// unchanged plugins does not compile them again; `None` compiles in memory only - no cache is
/// read or written (`plugins.compileCache`, RB-094-30).
///
/// Called before each load by the app; a process that never calls it - the packer, the tests -
/// compiles in memory. The plugins loaded before keep their engine and its choice.
pub fn set_compile_cache(directory: Option<&Path>) {
    SHARED.set_cache(directory);
}

/// The engine the next plugin is compiled on: the process's one, built anew when the compile
/// cache choice changed since.
pub(crate) fn shared() -> Result<Arc<SharedEngine>> {
    SHARED.get()
}

/// The engine new plugins are compiled on and the compile cache it is built for.
pub(crate) struct EngineSlot {
    state: Mutex<SlotState>,
}

struct SlotState {
    /// The cache folder the next engine takes; `None` compiles in memory only.
    wanted: Option<PathBuf>,
    /// The engine handed out now, with the cache folder it was built for.
    engine: Option<(Option<PathBuf>, Arc<SharedEngine>)>,
}

impl EngineSlot {
    pub(crate) const fn new() -> Self {
        Self {
            state: Mutex::new(SlotState {
                wanted: None,
                engine: None,
            }),
        }
    }

    /// Chooses the cache of the engines handed out from now on.
    pub(crate) fn set_cache(&self, directory: Option<&Path>) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .wanted = directory.map(Path::to_path_buf);
    }

    /// The engine for the cache chosen now: the one handed out before while the choice is the
    /// same, else a new one. Built under the lock, so two first uses build one engine.
    pub(crate) fn get(&self) -> Result<Arc<SharedEngine>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("plugin engine lock poisoned"))?;
        if let Some((built_for, engine)) = &state.engine
            && *built_for == state.wanted
        {
            return Ok(Arc::clone(engine));
        }
        let engine = Arc::new(SharedEngine::new(state.wanted.as_deref())?);
        state.engine = Some((state.wanted.clone(), Arc::clone(&engine)));
        Ok(engine)
    }
}

/// An engine, its epoch ticker, and what it has compiled.
pub(crate) struct SharedEngine {
    engine: Engine,
    compiled: Mutex<HashMap<[u8; 32], Component>>,
    disk: Option<CompileCache>,
    /// Compiles that did not come from memory, for the tests that say "once".
    compilations: AtomicUsize,
    ticker_state: Arc<TickerState>,
    ticker: Option<JoinHandle<()>>,
}

/// What the epoch ticker and the running calls share.
#[derive(Default)]
struct TickerState {
    /// The calls running now ([`Ticking`]); the ticker parks at zero.
    active: AtomicUsize,
    stop: AtomicBool,
    /// Epochs advanced, for the tests that say "idle".
    ticks: AtomicU64,
}

/// Keeps the epoch moving while held: one per call's store ([`SharedEngine::ticking`]).
pub(crate) struct Ticking {
    state: Arc<TickerState>,
}

impl Drop for Ticking {
    fn drop(&mut self) {
        self.state.active.fetch_sub(1, Ordering::AcqRel);
    }
}

impl SharedEngine {
    /// Builds an engine with fuel and epoch interruption, and the disk cache when given one.
    pub(crate) fn new(cache_directory: Option<&Path>) -> Result<Self> {
        let mut config = Config::new();
        config
            .wasm_component_model(true)
            .consume_fuel(true)
            .epoch_interruption(true)
            .cranelift_nan_canonicalization(true);
        // Opened - and swept - before the engine exists, so nothing unchecked is ever read. A
        // cache that cannot be made the user's alone (a file system without ACLs, a folder of
        // another account) is not used: the plugins compile in memory instead.
        let disk = cache_directory.and_then(|directory| {
            CompileCache::open(directory)
                .inspect_err(|error| {
                    tracing::warn!("plugin compile cache off: {error:#}");
                })
                .ok()
        });
        if let Some(disk) = &disk {
            config.cache(Some(disk.cache().clone()));
        }
        let engine = Engine::new(&config)
            .map_err(|error| anyhow::anyhow!("create the Wasmtime plugin engine: {error}"))?;
        let ticker_state = Arc::new(TickerState::default());
        let ticker = spawn_epoch_ticker(engine.clone(), Arc::clone(&ticker_state));
        Ok(Self {
            engine,
            compiled: Mutex::new(HashMap::new()),
            disk,
            compilations: AtomicUsize::new(0),
            ticker_state,
            ticker: Some(ticker),
        })
    }

    /// The epoch moves while the answer is held; the first holder wakes the ticker.
    pub(crate) fn ticking(&self) -> Ticking {
        if self.ticker_state.active.fetch_add(1, Ordering::AcqRel) == 0 {
            self.wake_ticker();
        }
        Ticking {
            state: Arc::clone(&self.ticker_state),
        }
    }

    fn wake_ticker(&self) {
        if let Some(ticker) = &self.ticker {
            ticker.thread().unpark();
        }
    }

    pub(crate) fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Compiles `bytes` once per exact content and hands out the same component afterwards.
    ///
    /// The lock is not held across the compile, so two plugins compile side by side; two
    /// callers racing on the *same* bytes both compile and the first result is kept.
    pub(crate) fn compile(&self, bytes: &[u8]) -> Result<Component> {
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if let Some(component) = self.lock()?.get(&digest) {
            return Ok(component.clone());
        }
        let component = Component::from_binary(&self.engine, bytes)
            .map_err(|error| anyhow::anyhow!("compile WebAssembly component: {error}"))?;
        self.compilations.fetch_add(1, Ordering::Relaxed);
        if let Some(disk) = &self.disk {
            disk.record_new_entries();
        }
        let mut compiled = self.lock()?;
        if compiled.len() >= MAX_COMPILED {
            compiled.clear();
        }
        Ok(compiled.entry(digest).or_insert(component).clone())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<[u8; 32], Component>>> {
        self.compiled
            .lock()
            .map_err(|_| anyhow::anyhow!("plugin compile memo lock poisoned"))
    }

    #[cfg(test)]
    pub(crate) fn compilations(&self) -> usize {
        self.compilations.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn ticks(&self) -> u64 {
        self.ticker_state.ticks.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn disk(&self) -> Option<&CompileCache> {
        self.disk.as_ref()
    }
}

impl Drop for SharedEngine {
    fn drop(&mut self) {
        self.ticker_state.stop.store(true, Ordering::Release);
        self.wake_ticker();
        if let Some(ticker) = self.ticker.take() {
            let _ = ticker.join().log_panic("plugin_host.epoch_ticker");
        }
    }
}

/// Advances the epoch every tick while a call runs, parked otherwise. A wake between the look at
/// `active` and the park is not lost: `park` returns at once after an earlier `unpark`.
fn spawn_epoch_ticker(engine: Engine, state: Arc<TickerState>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        while !state.stop.load(Ordering::Acquire) {
            if state.active.load(Ordering::Acquire) == 0 {
                std::thread::park();
                continue;
            }
            std::thread::sleep(crate::runtime::EPOCH_TICK);
            engine.increment_epoch();
            state.ticks.fetch_add(1, Ordering::AcqRel);
        }
    })
}

#[cfg(test)]
#[path = "engine_tests.rs"]
pub(crate) mod tests;
