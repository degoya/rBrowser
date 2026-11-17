//! [`FakeEngine`]: an engine in memory. Its views keep a history, record input, emulation and
//! window moves, and can crash or be closed "by the user" on request.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rb_util::lock;

use async_trait::async_trait;
use rb_config::EngineKind;
use tokio::sync::broadcast;

use crate::emulation_report::EmulationReport;
use crate::engine::{
    CellApplied, Engine, EngineHealth, HandleEvent, HandleEvents, InputEvent, Screenshot,
    ScreenshotRequest, ViewHandle, ViewSpec,
};
use crate::external::{CellScale, NativeWindow, WindowRect};
use crate::{Emulation, EngineError, ViewId};

/// The PNG signature, base64: what fake screenshots answer.
pub const FAKE_PNG: &str = "iVBORw0KGgo=";

/// An engine in memory.
pub struct FakeEngine {
    kind: EngineKind,
    external: bool,
    health: Mutex<EngineHealth>,
    views: Mutex<Vec<Arc<FakeView>>>,
    next: AtomicU32,
    fail_next: Mutex<Option<String>>,
    shutdowns: AtomicU32,
    /// The process its new views name as their window's owner (RB-080-39); `None` names no
    /// window.
    process_id: Mutex<Option<u32>>,
    /// While set, every `create_view` waits for a permit ([`FakeEngine::hold_starts`]).
    gate: Mutex<Option<Arc<tokio::sync::Semaphore>>>,
    /// The `create_view` calls waiting at the gate or past it.
    entered: AtomicU32,
}

impl FakeEngine {
    /// A stopped engine of `kind`; its views are external unless it is Chromium.
    pub fn new(kind: EngineKind) -> Arc<Self> {
        Arc::new(Self {
            kind,
            external: kind != EngineKind::Chromium,
            health: Mutex::new(EngineHealth::Stopped),
            views: Mutex::new(Vec::new()),
            next: AtomicU32::new(100),
            fail_next: Mutex::new(None),
            shutdowns: AtomicU32::new(0),
            process_id: Mutex::new(None),
            gate: Mutex::new(None),
            entered: AtomicU32::new(0),
        })
    }

    /// The process every view opened from now on names as its window's owner
    /// (`ViewHandle::native_window`, RB-080-39); `None` is an engine without a known window.
    pub fn set_process_id(&self, process_id: Option<u32>) {
        *lock(&self.process_id) = process_id;
    }

    /// Every view it opened, in order (closed ones too).
    pub fn views(&self) -> Vec<Arc<FakeView>> {
        lock(&self.views).clone()
    }

    /// The newest view with id `view`.
    pub fn view(&self, view: ViewId) -> Option<Arc<FakeView>> {
        lock(&self.views)
            .iter()
            .rev()
            .find(|fake| fake.id == view)
            .cloned()
    }

    /// Every `create_view` from now on waits until [`FakeEngine::let_starts_go`]: a test holds
    /// a start half-way to pin what happens meanwhile.
    pub fn hold_starts(&self) {
        *lock(&self.gate) = Some(Arc::new(tokio::sync::Semaphore::new(0)));
    }

    /// Lets every held and later `create_view` go on.
    pub fn let_starts_go(&self) {
        if let Some(gate) = lock(&self.gate).take() {
            gate.close();
        }
    }

    /// How many `create_view` calls began (held ones included).
    pub fn starts_entered(&self) -> u32 {
        self.entered.load(Ordering::Acquire)
    }

    /// The next `create_view` fails with `reason`.
    pub fn fail_next_start(&self, reason: &str) {
        *lock(&self.fail_next) = Some(reason.to_owned());
    }

    /// How it reports itself from now on.
    pub fn set_health(&self, health: EngineHealth) {
        *lock(&self.health) = health;
    }

    /// How often it was shut down.
    pub fn shutdowns(&self) -> u32 {
        self.shutdowns.load(Ordering::Acquire)
    }
}

#[async_trait]
impl Engine for FakeEngine {
    fn kind(&self) -> EngineKind {
        self.kind
    }

    fn external(&self) -> bool {
        self.external
    }

    fn health(&self) -> EngineHealth {
        lock(&self.health).clone()
    }

    async fn create_view(&self, spec: &ViewSpec) -> Result<Arc<dyn ViewHandle>, EngineError> {
        self.entered.fetch_add(1, Ordering::AcqRel);
        let gate = lock(&self.gate).clone();
        if let Some(gate) = gate {
            // Closed by `let_starts_go`: the error is the signal to go on.
            let _released = gate.acquire().await;
        }
        if let Some(reason) = lock(&self.fail_next).take() {
            return Err(EngineError::Unavailable {
                engine: self.kind,
                reason,
            });
        }
        let id = spec
            .view
            .unwrap_or_else(|| ViewId(self.next.fetch_add(1, Ordering::AcqRel)));
        let (events, _) = broadcast::channel(64);
        let view = Arc::new(FakeView {
            id,
            spec: spec.clone(),
            kind: self.kind,
            external: self.external,
            process_id: *lock(&self.process_id),
            state: Mutex::new(FakeState {
                history: if spec.url.is_empty() {
                    Vec::new()
                } else {
                    vec![spec.url.clone()]
                },
                index: 0,
                emulation: Some(spec.emulation.clone()),
                inputs: Vec::new(),
                reloads: 0,
                window: spec.window,
                raised: 0,
                closed: false,
                cells: Vec::new(),
                fail_cell: None,
            }),
            events,
        });
        lock(&self.views).push(view.clone());
        *lock(&self.health) = EngineHealth::Running;
        Ok(view)
    }

    async fn shutdown(&self) {
        for view in self.views() {
            view.end(false);
        }
        *lock(&self.health) = EngineHealth::Stopped;
        self.shutdowns.fetch_add(1, Ordering::AcqRel);
    }
}

#[derive(Debug)]
struct FakeState {
    history: Vec<String>,
    index: usize,
    emulation: Option<Emulation>,
    inputs: Vec<InputEvent>,
    reloads: u32,
    window: Option<WindowRect>,
    raised: u32,
    closed: bool,
    /// Every scale asked of it (`ViewHandle::set_cell`, RB-080-40), in order.
    cells: Vec<Option<CellScale>>,
    /// The next `set_cell` fails with this reason, after recording the call.
    fail_cell: Option<String>,
}

/// One view of a [`FakeEngine`].
pub struct FakeView {
    id: ViewId,
    spec: ViewSpec,
    kind: EngineKind,
    external: bool,
    process_id: Option<u32>,
    state: Mutex<FakeState>,
    events: broadcast::Sender<HandleEvent>,
}

impl FakeView {
    /// The spec it was opened with.
    pub fn spec(&self) -> ViewSpec {
        self.spec.clone()
    }

    /// The current URL; empty before the first.
    pub fn url(&self) -> String {
        let state = lock(&self.state);
        state.history.get(state.index).cloned().unwrap_or_default()
    }

    /// The input replayed into it, in order.
    pub fn inputs(&self) -> Vec<InputEvent> {
        lock(&self.state).inputs.clone()
    }

    /// The emulation it has.
    pub fn emulation(&self) -> Option<Emulation> {
        lock(&self.state).emulation.clone()
    }

    /// How often it was reloaded.
    pub fn reloads(&self) -> u32 {
        lock(&self.state).reloads
    }

    /// Its window.
    pub fn window(&self) -> Option<WindowRect> {
        lock(&self.state).window
    }

    /// How often its window was raised.
    pub fn raised(&self) -> u32 {
        lock(&self.state).raised
    }

    /// Every scale asked of it, in order (RB-080-40).
    pub fn cells(&self) -> Vec<Option<CellScale>> {
        lock(&self.state).cells.clone()
    }

    /// The next `set_cell` is refused with `reason` (recorded all the same).
    pub fn fail_next_cell(&self, reason: &str) {
        lock(&self.state).fail_cell = Some(reason.to_owned());
    }

    /// The engine built its window anew (RB-080-40).
    pub fn replace_window(&self) {
        // Nobody listening is fine.
        let _sent = self.events.send(HandleEvent::WindowReplaced);
    }

    /// Whether it is closed.
    pub fn is_closed(&self) -> bool {
        lock(&self.state).closed
    }

    /// The engine dies under it.
    pub fn crash(&self) {
        self.end(true);
    }

    /// The user closes its window.
    pub fn close_by_user(&self) {
        self.end(false);
    }

    fn end(&self, crashed: bool) {
        let was_open = !std::mem::replace(&mut lock(&self.state).closed, true);
        if was_open {
            // Nobody listening is fine.
            let _sent = self.events.send(HandleEvent::Exited { crashed });
        }
    }

    fn state(&self) -> Result<MutexGuard<'_, FakeState>, EngineError> {
        let state = lock(&self.state);
        if state.closed {
            Err(EngineError::Closed)
        } else {
            Ok(state)
        }
    }

    fn external_only(&self, feature: &str) -> Result<MutexGuard<'_, FakeState>, EngineError> {
        let state = self.state()?;
        if self.external {
            Ok(state)
        } else {
            Err(EngineError::Unsupported {
                engine: self.kind,
                feature: feature.to_owned(),
            })
        }
    }

    fn committed(&self, url: String) {
        // Nobody listening is fine.
        let _sent = self.events.send(HandleEvent::Navigated { url });
    }
}

#[async_trait]
impl ViewHandle for FakeView {
    fn id(&self) -> ViewId {
        self.id
    }

    fn engine(&self) -> EngineKind {
        self.kind
    }

    fn is_external(&self) -> bool {
        self.external
    }

    async fn navigate(&self, url: &str) -> Result<(), EngineError> {
        {
            let mut state = self.state()?;
            let keep = if state.history.is_empty() {
                0
            } else {
                state.index + 1
            };
            state.history.truncate(keep);
            state.history.push(url.to_owned());
            state.index = state.history.len() - 1;
        }
        self.committed(url.to_owned());
        Ok(())
    }

    async fn reload(&self, _ignore_cache: bool) -> Result<(), EngineError> {
        self.state()?.reloads += 1;
        Ok(())
    }

    async fn back(&self) -> Result<(), EngineError> {
        let url = {
            let mut state = self.state()?;
            if state.index == 0 {
                return Ok(());
            }
            state.index -= 1;
            state.history[state.index].clone()
        };
        self.committed(url);
        Ok(())
    }

    async fn forward(&self) -> Result<(), EngineError> {
        let url = {
            let mut state = self.state()?;
            if state.index + 1 >= state.history.len() {
                return Ok(());
            }
            state.index += 1;
            state.history[state.index].clone()
        };
        self.committed(url);
        Ok(())
    }

    async fn apply_emulation(&self, emulation: &Emulation) -> Result<EmulationReport, EngineError> {
        self.state()?.emulation = Some(emulation.clone());
        Ok(EmulationReport::for_engine(self.kind, emulation))
    }

    async fn dispatch_input(&self, input: &InputEvent) -> Result<(), EngineError> {
        self.state()?.inputs.push(input.clone());
        Ok(())
    }

    async fn screenshot(&self, _request: ScreenshotRequest) -> Result<Screenshot, EngineError> {
        drop(self.state()?);
        Ok(Screenshot {
            png_base64: FAKE_PNG.to_owned(),
        })
    }

    async fn set_window(&self, rect: WindowRect) -> Result<(), EngineError> {
        self.external_only("window placement")?.window = Some(rect);
        Ok(())
    }

    async fn raise(&self) -> Result<(), EngineError> {
        self.external_only("raising a window")?.raised += 1;
        Ok(())
    }

    async fn set_cell(&self, scale: Option<CellScale>) -> Result<CellApplied, EngineError> {
        let mut state = self.external_only("scaling into the cell")?;
        state.cells.push(scale);
        match state.fail_cell.take() {
            Some(reason) => Err(EngineError::Invalid(reason)),
            None => Ok(CellApplied::default()),
        }
    }

    fn native_window(&self) -> Option<NativeWindow> {
        if !self.external {
            return None;
        }
        Some(NativeWindow {
            process_id: self.process_id?,
            window_class: None,
        })
    }

    fn events(&self) -> HandleEvents {
        HandleEvents::direct(self.events.subscribe())
    }

    async fn close(&self) -> Result<(), EngineError> {
        self.end(false);
        Ok(())
    }
}
