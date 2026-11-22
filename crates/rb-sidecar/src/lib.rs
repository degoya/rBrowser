//! The Node sidecar (RB-040-03): WebKit (RB-040-04), flows (RB-040-07), Lighthouse (RB-060-07)
//! and the recorder export test (RB-050-02) run in Node with Playwright; the Chromium core,
//! Firefox, sync, MCP and screenshots never need it, and nothing here starts Node before the first
//! request.
//!
//! - [`node`]: which runtime - `engines.nodePath`, else the bundled `<install>/node/node.exe`
//!   (`node/bin/node` on macOS) - and its major version against `.nvmrc`.
//! - [`layout`]: where the package keeps the runtime and the sidecar - beside the executable, or
//!   in `rBrowser.app/Contents/Resources` on macOS (RB-097-01).
//! - [`rpc`] and [`client`]: JSON-RPC 2.0 over stdio, one message per line; requests with a
//!   timeout, `$/progress` notifications, `$/cancel` when a request is dropped or times out.
//! - [`supervisor`]: the process' life - start on first need, health ping, restart with backoff
//!   after a crash, clean shutdown; [`process`] starts the real one (`NoConsoleWindow`, stderr
//!   into `tracing` with secrets masked).
//! - [`playwright`]: the browser download into `%LOCALAPPDATA%\rBrowser\ms-playwright`
//!   (`~/.local/state/rBrowser/ms-playwright` on macOS).
//! - [`command`]: the commands `sidecar.restart` and `engines.update`.

pub mod backoff;
pub mod client;
pub mod command;
pub mod error;
pub mod framing;
pub mod layout;
pub mod node;
pub mod playwright;
pub mod process;
pub mod rpc;
pub mod settings;
pub mod supervisor;

pub use client::RpcClient;
pub use error::SidecarError;
pub use process::{ConfiguredLauncher, NodeLauncher};
pub use settings::SidecarSettings;
pub use supervisor::{Launched, Launcher, STOP_GRACE, Sidecar, Subscription};

/// The `tracing` target of everything the sidecar logs, its stderr included.
pub const LOG_TARGET: &str = "sidecar";
