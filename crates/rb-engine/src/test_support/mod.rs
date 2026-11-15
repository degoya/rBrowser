//! Test doubles every engine job uses: [`FakeEngine`] (an engine in memory), [`FakeHost`] (the
//! CEF app's [`crate::ViewHost`] in memory, under [`crate::chromium::ChromiumEngine`]) and the
//! conformance checks every engine must pass ([`conformance`]; RB-040-02 and RB-040-04 run
//! them against Firefox and WebKit).

pub mod conformance;
mod fake_engine;
mod fake_host;

pub use fake_engine::{FAKE_PNG, FakeEngine, FakeView};
pub use fake_host::{CdpCall, FakeHost};
