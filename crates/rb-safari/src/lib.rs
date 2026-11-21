//! Safari on macOS (RB-095-03): a lean W3C WebDriver client over HTTP to `safaridriver`
//! ([`http`], [`protocol`], [`client`]), the driver as a child process ([`driver`]), what only a
//! Mac has ([`platform`]), and the Safari engine behind `rb_engine::Engine` ([`engine`],
//! [`view`]) with the sync agent booted on demand ([`agent`]), the viewport through the window
//! ([`emulation`]) and the SyncHub's replay ([`replay`]). The same engine runs iOS views on the
//! iOS Simulator or a connected iPhone or iPad ([`ios`], RB-095-04).
//!
//! | Part | Where |
//! | --- | --- |
//! | HTTP/1.1 to `127.0.0.1` only, plain path segments, untrusted answers | [`http`] |
//! | WebDriver's answers and errors; remote automation off; another session | [`protocol`], [`SafariError`] |
//! | The commands per session | [`client`] |
//! | `safaridriver --port` on a free loopback port, no console window | [`driver`] |
//! | `SafariEngine`, `SafariView`, `SafariReplay` | [`engine`], [`view`], [`replay`] |
//! | iOS: Xcode's simulators, devices on USB, the pick of one, touch (RB-095-04) | [`ios`] |
//! | A fake `safaridriver` over real HTTP, for tests (feature `test-support`) | `testing` |
//!
//! Safari runs only on macOS ([`platform::SUPPORTED`]); everything here builds everywhere, so
//! the fake drives the engine on Linux and Windows too. No log line, error or `Debug` output
//! carries a request body or an answer: a replayed password travels in them.

pub mod agent;
pub mod client;
pub mod driver;
pub mod emulation;
pub mod engine;
mod error;
pub mod http;
pub mod ios;
pub mod platform;
pub mod protocol;
pub mod replay;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;
pub mod view;

pub use client::{DriverClient, Session};
pub use driver::{DriverLaunch, Edition, Launch};
pub use engine::{SafariConfig, SafariEngine};
pub use error::{SafariError, engine_error, engine_error_for};
pub use platform::SUPPORTED;
pub use replay::SafariReplay;
