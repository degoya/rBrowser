//! WebDriver BiDi and Firefox (RB-040-02, ADR 0001 Decisions 7 and 8): a lean client over
//! tokio-tungstenite ([`client`]), the commands rBrowser sends per domain
//! ([`browsing_context`], [`script`], [`input`], [`emulation`], [`storage`], [`browser`]), the
//! launcher of the installed Firefox ([`launcher`]) with its own profile ([`profile`]), and the
//! Firefox engine behind `rb_engine::Engine` ([`firefox`]).
//!
//! | Part | Where |
//! | --- | --- |
//! | Frames in and out, untrusted input | [`protocol`] |
//! | Id correlation, events, timeouts, loopback only | [`client`] |
//! | Find, start and stop Firefox (`--remote-debugging-port 0`, stderr endpoint) | [`launcher`] |
//! | The profile folder (no links, marker), `user.js` | [`profile`] |
//! | `FirefoxEngine`, its views, the sync agent in a sandbox, the replay | [`firefox`] |
//! | A fake Firefox over a real WebSocket, for tests (feature `test-support`) | `testing` |
//!
//! No log line, error or `Debug` output carries a command's parameters or a frame Firefox sent:
//! a replayed password field travels in them.

pub mod browser;
pub mod browsing_context;
pub mod client;
pub mod emulation;
mod error;
pub mod firefox;
pub mod input;
pub mod launcher;
pub mod profile;
pub mod protocol;
pub mod script;
pub mod storage;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use client::{BidiClient, DEFAULT_TIMEOUT};
pub use error::BidiError;
pub use firefox::{FirefoxConfig, FirefoxEngine, FirefoxReplay};
pub use protocol::{Event, Failure};
