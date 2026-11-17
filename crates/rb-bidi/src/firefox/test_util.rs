//! The helpers the Firefox engine's tests share.

use std::sync::Arc;

use rb_sync::{AgentConfig, CaptureSwitches};
use tokio::runtime::Handle;

use super::{FirefoxConfig, FirefoxEngine};
use crate::testing::FakeFirefox;

pub fn agent_config() -> AgentConfig {
    AgentConfig {
        active: true,
        capture: CaptureSwitches {
            click: true,
            hover: true,
            scroll: true,
            keyboard: true,
            input: true,
            forms: true,
            focus: false,
        },
        hover_throttle_ms: 100,
        scroll_debounce_ms: rb_sync::message::DEFAULT_SCROLL_DEBOUNCE_MS,
        scroll_mode: rb_sync::AgentScrollMode::Anchor,
    }
}

pub fn engine(fake: &FakeFirefox, agent: &str) -> Arc<FirefoxEngine> {
    FirefoxEngine::new(
        FirefoxConfig {
            launch: fake.launcher(),
            agent_source: Arc::from(agent),
            agent_config: agent_config(),
        },
        Handle::current(),
    )
}
