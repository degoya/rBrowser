//! A [`ViewHost`] for the tests of hot reload: views of a given engine, each answering the HMR
//! probe and the swap as told; reloads and scripts are recorded.

use std::collections::BTreeMap;
use std::sync::Mutex;

use rb_util::lock;

use async_trait::async_trait;
use rb_config::{Breakpoint, DevtoolsMode, EngineKind, Layout};
use rb_engine::{Emulation, EngineError, Target, ViewEvent, ViewHost, ViewId, ViewInfo};
use serde_json::{Value, json};
use tokio::sync::broadcast;

use super::script::{HMR_PROBE, TYPING_PROBE};

/// How a fake view answers.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Page {
    /// The engine.
    pub engine: EngineKind,
    /// The HMR probe says yes.
    pub hmr: bool,
    /// The swap's count.
    pub swaps: u64,
    /// Every script throws.
    pub broken: bool,
    /// The typing probe says yes.
    pub typing: bool,
    /// No isolated world can be made.
    pub no_world: bool,
}

pub(crate) struct FakeHost {
    pages: BTreeMap<ViewId, Page>,
    pub reloads: Mutex<Vec<(ViewId, bool)>>,
    pub scripts: Mutex<Vec<(ViewId, String)>>,
    /// Every CDP call: view, method, parameters.
    pub cdp: Mutex<Vec<(ViewId, String, Value)>>,
    events: broadcast::Sender<ViewEvent>,
}

impl FakeHost {
    pub(crate) fn new(pages: &[Page]) -> Self {
        let pages = pages
            .iter()
            .enumerate()
            .map(|(index, page)| {
                let id = u32::try_from(index + 1).expect("few test views");
                (ViewId(id), *page)
            })
            .collect();
        Self {
            pages,
            reloads: Mutex::new(Vec::new()),
            scripts: Mutex::new(Vec::new()),
            cdp: Mutex::new(Vec::new()),
            events: broadcast::channel(16).0,
        }
    }

    pub(crate) fn reloaded(&self) -> Vec<ViewId> {
        lock(&self.reloads).iter().map(|(view, _)| *view).collect()
    }

    pub(crate) fn calls(&self) -> Vec<(ViewId, String, Value)> {
        lock(&self.cdp).clone()
    }

    pub(crate) fn swaps_sent(&self) -> usize {
        lock(&self.scripts)
            .iter()
            .filter(|(_, script)| script.contains("Swaps changed stylesheets"))
            .count()
    }

    fn info(id: ViewId, page: &Page) -> ViewInfo {
        ViewInfo {
            id,
            label: format!("view {id}"),
            breakpoint: None,
            cell_breakpoint: None,
            engine: page.engine,
            width: 400,
            height: 800,
            dpr: 1.0,
            scale: 1.0,
            url: "http://localhost:5173/".into(),
            title: String::new(),
            loading: false,
            session: "default".into(),
            cell: None,
        }
    }
}

#[async_trait]
impl ViewHost for FakeHost {
    fn views(&self) -> Vec<ViewInfo> {
        self.pages
            .iter()
            .map(|(id, page)| Self::info(*id, page))
            .collect()
    }

    fn view(&self, view: ViewId) -> Option<ViewInfo> {
        self.pages.get(&view).map(|page| Self::info(view, page))
    }

    fn active_view(&self) -> Option<ViewId> {
        None
    }

    fn set_active_view(&self, _: ViewId) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented)
    }

    fn set_breakpoint(&self, _: ViewId, _: &Breakpoint) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented)
    }

    fn set_size(&self, _: ViewId, _: u32, _: u32, _: Option<f64>) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented)
    }

    fn apply_emulation(&self, _: ViewId, _: &Emulation) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented)
    }

    fn emulation(&self, _: ViewId) -> Option<Emulation> {
        None
    }

    fn open_view(&self, _: &Breakpoint) -> Result<ViewId, EngineError> {
        Err(EngineError::NotImplemented)
    }

    fn set_layout(&self, _: &Layout) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented)
    }

    fn navigate(&self, _: Target, _: &str) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented)
    }

    fn reload(&self, target: Target, ignore_cache: bool) -> Result<(), EngineError> {
        let views: Vec<ViewId> = match target {
            Target::All => self.pages.keys().copied().collect(),
            Target::View(view) => vec![view],
        };
        for view in views {
            lock(&self.reloads).push((view, ignore_cache));
        }
        Ok(())
    }

    fn go_back(&self, _: Target) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented)
    }

    fn go_forward(&self, _: Target) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented)
    }

    fn open_devtools(&self, _: ViewId, _: DevtoolsMode) -> Result<(), EngineError> {
        Err(EngineError::NotImplemented)
    }

    async fn cdp_send(
        &self,
        view: ViewId,
        method: &str,
        params: Value,
    ) -> Result<Value, EngineError> {
        let page = self
            .pages
            .get(&view)
            .ok_or(EngineError::UnknownView(view))?;
        lock(&self.cdp).push((view, method.to_owned(), params.clone()));
        match method {
            "Page.getFrameTree" => {
                return Ok(json!({ "frameTree": { "frame": { "id": format!("F{}", view.0) } } }));
            }
            "Page.createIsolatedWorld" if page.no_world => {
                return Err(EngineError::Invalid("no world".into()));
            }
            "Page.createIsolatedWorld" => {
                return Ok(json!({ "executionContextId": 100 + i64::from(view.0) }));
            }
            _ => assert_eq!(method, "Runtime.evaluate"),
        }
        let script = params["expression"].as_str().unwrap_or_default().to_owned();
        lock(&self.scripts).push((view, script.clone()));
        if page.broken {
            return Ok(json!({ "result": {}, "exceptionDetails": { "text": "boom" } }));
        }
        let value = if script == HMR_PROBE {
            json!(page.hmr)
        } else if script == TYPING_PROBE {
            json!(page.typing)
        } else if script.contains("Swaps changed stylesheets") {
            json!(page.swaps)
        } else {
            json!(false)
        };
        Ok(json!({ "result": { "value": value } }))
    }

    fn subscribe(&self) -> broadcast::Receiver<ViewEvent> {
        self.events.subscribe()
    }
}
