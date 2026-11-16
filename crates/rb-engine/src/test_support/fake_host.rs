//! [`FakeHost`]: a [`ViewHost`] in memory - the CEF app as [`crate::chromium::ChromiumEngine`]
//! sees it. It records every call and announces navigations like the real host.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use rb_util::lock;

use async_trait::async_trait;
use rb_config::{Breakpoint, DevtoolsMode, EngineKind, Layout};
use serde_json::{Value, json};
use tokio::sync::broadcast;

use super::fake_engine::FAKE_PNG;
use crate::{Emulation, EngineError, Target, ViewEvent, ViewHost, ViewId, ViewInfo};

/// One CDP command a test sent.
#[derive(Clone, Debug, PartialEq)]
pub struct CdpCall {
    /// The view.
    pub view: ViewId,
    /// The method.
    pub method: String,
    /// The parameters.
    pub params: Value,
}

/// The CEF app in memory.
pub struct FakeHost {
    views: Mutex<Vec<ViewInfo>>,
    active: Mutex<Option<ViewId>>,
    emulation: Mutex<HashMap<ViewId, Emulation>>,
    calls: Mutex<Vec<String>>,
    cdp: Mutex<Vec<CdpCall>>,
    next: AtomicU32,
    events: broadcast::Sender<ViewEvent>,
}

impl FakeHost {
    /// A host without views.
    pub fn new() -> Arc<Self> {
        let (events, _) = broadcast::channel(256);
        Arc::new(Self {
            views: Mutex::new(Vec::new()),
            active: Mutex::new(None),
            emulation: Mutex::new(HashMap::new()),
            calls: Mutex::new(Vec::new()),
            cdp: Mutex::new(Vec::new()),
            next: AtomicU32::new(1),
            events,
        })
    }

    /// Adds a view labelled `label` running on `engine`; answers its id.
    pub fn add_view(&self, label: &str, engine: EngineKind) -> ViewId {
        let id = ViewId(self.next.fetch_add(1, Ordering::AcqRel));
        lock(&self.views).push(ViewInfo {
            id,
            label: label.to_owned(),
            breakpoint: Some(label.to_owned()),
            cell_breakpoint: Some(label.to_owned()),
            engine,
            width: 400,
            height: 800,
            dpr: 1.0,
            scale: 1.0,
            url: String::new(),
            title: String::new(),
            loading: false,
            session: "default".to_owned(),
            cell: None,
        });
        id
    }

    /// The calls besides CDP, as `"<method> <view> <argument>"`, in order.
    pub fn calls(&self) -> Vec<String> {
        lock(&self.calls).clone()
    }

    /// The CDP commands, in order.
    pub fn cdp_calls(&self) -> Vec<CdpCall> {
        lock(&self.cdp).clone()
    }

    /// Announces `event` to the subscribers.
    pub fn emit(&self, event: ViewEvent) {
        // Nobody listening is fine.
        let _sent = self.events.send(event);
    }

    fn record(&self, call: String) {
        lock(&self.calls).push(call);
    }

    fn targets(&self, target: Target) -> Result<Vec<ViewId>, EngineError> {
        let views = lock(&self.views);
        match target {
            Target::All => Ok(views.iter().map(|view| view.id).collect()),
            Target::View(id) if views.iter().any(|view| view.id == id) => Ok(vec![id]),
            Target::View(id) => Err(EngineError::UnknownView(id)),
        }
    }

    fn known(&self, view: ViewId) -> Result<(), EngineError> {
        self.targets(Target::View(view)).map(|_| ())
    }

    fn each(&self, method: &str, target: Target, argument: &str) -> Result<(), EngineError> {
        for view in self.targets(target)? {
            self.record(format!("{method} {view} {argument}"));
        }
        Ok(())
    }
}

#[async_trait]
impl ViewHost for FakeHost {
    fn views(&self) -> Vec<ViewInfo> {
        lock(&self.views).clone()
    }

    fn view(&self, view: ViewId) -> Option<ViewInfo> {
        lock(&self.views)
            .iter()
            .find(|info| info.id == view)
            .cloned()
    }

    fn active_view(&self) -> Option<ViewId> {
        *lock(&self.active)
    }

    fn set_active_view(&self, view: ViewId) -> Result<(), EngineError> {
        self.known(view)?;
        *lock(&self.active) = Some(view);
        self.emit(ViewEvent::ActiveChanged(Some(view)));
        Ok(())
    }

    fn set_breakpoint(&self, view: ViewId, breakpoint: &Breakpoint) -> Result<(), EngineError> {
        self.known(view)?;
        self.record(format!("set_breakpoint {view} {}", breakpoint.id));
        Ok(())
    }

    fn set_size(
        &self,
        view: ViewId,
        width: u32,
        height: u32,
        _dpr: Option<f64>,
    ) -> Result<(), EngineError> {
        self.known(view)?;
        self.record(format!("set_size {view} {width}x{height}"));
        Ok(())
    }

    fn apply_emulation(&self, view: ViewId, emulation: &Emulation) -> Result<(), EngineError> {
        self.known(view)?;
        self.record(format!(
            "apply_emulation {view} {}x{}",
            emulation.width, emulation.height
        ));
        lock(&self.emulation).insert(view, emulation.clone());
        Ok(())
    }

    fn emulation(&self, view: ViewId) -> Option<Emulation> {
        lock(&self.emulation).get(&view).cloned()
    }

    fn open_view(&self, breakpoint: &Breakpoint) -> Result<ViewId, EngineError> {
        let label = breakpoint.label.as_deref().unwrap_or(&breakpoint.id);
        let view = self.add_view(label, breakpoint.engine);
        self.emit(ViewEvent::Opened(view));
        Ok(view)
    }

    fn set_layout(&self, layout: &Layout) -> Result<(), EngineError> {
        self.record(format!("set_layout - {}", layout.cells.len()));
        Ok(())
    }

    fn navigate(&self, target: Target, url: &str) -> Result<(), EngineError> {
        for view in self.targets(target)? {
            self.record(format!("navigate {view} {url}"));
            if let Some(info) = lock(&self.views).iter_mut().find(|info| info.id == view) {
                url.clone_into(&mut info.url);
            }
            self.emit(ViewEvent::Navigated {
                view,
                url: url.to_owned(),
            });
        }
        Ok(())
    }

    fn reload(&self, target: Target, ignore_cache: bool) -> Result<(), EngineError> {
        self.each("reload", target, if ignore_cache { "hard" } else { "soft" })
    }

    fn go_back(&self, target: Target) -> Result<(), EngineError> {
        self.each("go_back", target, "-")
    }

    fn go_forward(&self, target: Target) -> Result<(), EngineError> {
        self.each("go_forward", target, "-")
    }

    fn open_devtools(&self, view: ViewId, _mode: DevtoolsMode) -> Result<(), EngineError> {
        self.known(view)?;
        self.record(format!("open_devtools {view} -"));
        Ok(())
    }

    async fn cdp_send(
        &self,
        view: ViewId,
        method: &str,
        params: Value,
    ) -> Result<Value, EngineError> {
        self.known(view)?;
        lock(&self.cdp).push(CdpCall {
            view,
            method: method.to_owned(),
            params,
        });
        Ok(match method {
            "Page.captureScreenshot" => json!({ "data": FAKE_PNG }),
            _ => json!({}),
        })
    }

    fn subscribe(&self) -> broadcast::Receiver<ViewEvent> {
        self.events.subscribe()
    }
}
