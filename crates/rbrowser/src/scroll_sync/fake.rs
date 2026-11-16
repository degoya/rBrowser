//! The scroll tests' [`ReplayPort`]: a scroll mode, one answer of the agent's world for every
//! evaluation, and a record of the evaluations and the commands sent.

use std::sync::Mutex;

use async_trait::async_trait;
use rb_config::ScrollMode;
use rb_engine::{EngineError, ViewId};
use serde_json::{Value, json};

use crate::cdp::lock;
use crate::replay::{ReplayPort, ViewTraits};

pub(crate) struct ScrollPort {
    pub mode: ScrollMode,
    pub answer: Mutex<Value>,
    pub worlds: Mutex<Vec<(ViewId, String)>>,
    pub sends: Mutex<Vec<(ViewId, &'static str, Value)>>,
}

impl ScrollPort {
    /// A port in `mode` whose agents answer `answer`.
    pub(crate) fn new(mode: ScrollMode, answer: Value) -> Self {
        Self {
            mode,
            answer: Mutex::new(answer),
            worlds: Mutex::new(Vec::new()),
            sends: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn worlds(&self) -> Vec<(ViewId, String)> {
        lock(&self.worlds).clone()
    }

    pub(crate) fn sends(&self) -> Vec<(ViewId, &'static str, Value)> {
        lock(&self.sends).clone()
    }
}

#[async_trait]
impl ReplayPort for ScrollPort {
    fn traits(&self, view: ViewId) -> Option<ViewTraits> {
        (view.0 < 100).then(ViewTraits::default)
    }

    fn context_menu(&self) -> bool {
        false
    }

    fn scroll_mode(&self) -> ScrollMode {
        self.mode
    }

    async fn world(&self, view: ViewId, expression: String) -> Result<Value, EngineError> {
        lock(&self.worlds).push((view, expression));
        Ok(lock(&self.answer).clone())
    }

    async fn send(
        &self,
        view: ViewId,
        method: &'static str,
        params: Value,
    ) -> Result<Value, EngineError> {
        lock(&self.sends).push((view, method, params));
        Ok(json!({}))
    }
}
