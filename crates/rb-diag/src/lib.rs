//! Logging and diagnostics of rBrowser (RB-010-10), with no telemetry: everything stays on the
//! machine and leaves it only by hand.
//!
//! - [`logging`]: `tracing` into `%LOCALAPPDATA%\rBrowser\logs\rbrowser.log`, rotated by size,
//!   masked, level from the setting `logs.level` or `RUST_LOG`; one target per area
//!   ([`target`]). [`init`] also installs the panic hook ([`panic`]).
//! - [`redact`]: masks tokens, cookies, `Authorization` and config header values (`***`);
//!   [`page_text`]: the same for what a web page writes, credential URL parameters included.
//! - [`bundle`]: the diagnostics ZIP; [`command`]: `diagnostics.create`, the CLI arguments.
//! - [`cef`]: the CEF age notice, `crash_reporter.cfg` (local Crashpad, no upload) and CEF's log
//!   severity - data the CEF app (RB-010-04) applies.
//! - [`settings`]: the `logs` settings struct and the keys it reads.

pub mod bundle;
pub mod cef;
pub mod command;
mod error;
pub mod logging;
pub mod page_text;
pub mod panic;
pub mod paths;
pub mod redact;
mod reveal;
pub mod rotate;
pub mod settings;

pub use bundle::{BundleInput, BundleReport, OsInfo, Versions, write_bundle};
pub use cef::{CefAgeNotice, CefBuild, CefLogSeverity, cef_age_notice, crash_reporter_cfg};
pub use command::{DiagnosticsArgs, command_def};
pub use error::DiagError;
pub use logging::{LogConfig, LogGuard, LogHandle, init, target};
pub use paths::DiagPaths;
pub use redact::{redact, register_secret, unregister_secret};
pub use reveal::reveal;
pub use settings::{LogLevel, LogSettings};
