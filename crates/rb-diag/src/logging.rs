//! The process's log: `tracing` into `rbrowser.log`, rotated by size (`rotate`), every line
//! masked (`redact`) and written off the caller's thread by `tracing-appender`'s non-blocking
//! writer. The level comes from the setting `logs.level` and changes at runtime
//! ([`LogHandle::set_level`], the settings window; `logs.level` is protected from MCP);
//! `RUST_LOG`, when set, wins over it. Without `RUST_LOG`, rmcp stays at warn: it logs whole
//! tool calls and answers at debug.

use std::io::{self, Write};
use std::path::PathBuf;

use tracing::Dispatch;
use tracing_appender::non_blocking::{NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::{EnvFilter, Registry, reload};

use crate::error::DiagError;
use crate::paths::{DiagPaths, LOG_FILE_NAME};
use crate::rotate::{RotatingFile, RotationPolicy};
use crate::settings::{LogLevel, LogSettings};

/// The log targets of the areas, so `RUST_LOG=cdp=debug` picks one:
/// `tracing::info!(target: rb_diag::target::CDP, ...)`.
pub mod target {
    /// The core: config, views, the window, commands.
    pub const CORE: &str = "core";
    /// The CDP client and proxy.
    pub const CDP: &str = "cdp";
    /// The sync hub.
    pub const SYNC: &str = "sync";
    /// The MCP server.
    pub const MCP: &str = "mcp";
    /// The plugin host and plugins.
    pub const PLUGINS: &str = "plugins";
}

/// What the logger needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogConfig {
    /// The log directory.
    pub dir: PathBuf,
    /// The level when `RUST_LOG` is not set.
    pub level: LogLevel,
    /// Size and number of the files.
    pub rotation: RotationPolicy,
}

impl LogConfig {
    /// The config of `paths` and the `logs` settings.
    pub fn new(paths: &DiagPaths, settings: &LogSettings) -> Self {
        Self {
            dir: paths.logs.clone(),
            level: settings.level,
            rotation: settings.rotation(),
        }
    }
}

/// Dependencies that log whole payloads below warn, capped at warn unless `RUST_LOG` names
/// them: rmcp logs every MCP request and answer at debug (page content, `evaluate` results,
/// settings values). hyper, axum, tungstenite and ureq log through `log` or behind cfg flags
/// this logger does not take, so they never reach the file.
const CAPPED_AT_WARN: &[&str] = &["rmcp"];

/// The filter directives: `RUST_LOG` when it says anything, else the level, with the
/// dependencies of [`CAPPED_AT_WARN`] held at warn.
pub fn filter_directives(level: LogLevel, rust_log: Option<&str>) -> String {
    if let Some(directives) = rust_log.map(str::trim).filter(|text| !text.is_empty()) {
        return directives.to_owned();
    }
    let mut directives = level.as_str().to_owned();
    if level > LogLevel::Warn {
        directives.extend(
            CAPPED_AT_WARN
                .iter()
                .map(|target| format!(",{target}=warn")),
        );
    }
    directives
}

fn env_filter(level: LogLevel, rust_log: Option<&str>) -> EnvFilter {
    EnvFilter::try_new(filter_directives(level, rust_log))
        .unwrap_or_else(|_| EnvFilter::new(filter_directives(level, None)))
}

/// Changes the level of a running logger.
#[derive(Clone, Debug)]
pub struct LogHandle {
    filter: reload::Handle<EnvFilter, Registry>,
    rust_log: Option<String>,
}

impl LogHandle {
    /// Applies `level` (unless `RUST_LOG` was set at start, which keeps winning).
    pub fn set_level(&self, level: LogLevel) -> Result<(), DiagError> {
        self.filter
            .reload(env_filter(level, self.rust_log.as_deref()))
            .map_err(|error| DiagError::Filter(error.to_string()))
    }
}

/// Keeps the writer thread alive; dropping it flushes the lines still queued. Hold it in `main`
/// for the life of the process.
#[derive(Debug)]
pub struct LogGuard {
    _worker: WorkerGuard,
    handle: LogHandle,
}

impl LogGuard {
    /// The handle that changes the level.
    pub fn handle(&self) -> LogHandle {
        self.handle.clone()
    }
}

/// Builds the logger without installing it: the dispatcher and its guard. `rust_log` is the
/// value of `RUST_LOG`; an invalid one falls back to the level.
pub fn build(
    config: &LogConfig,
    rust_log: Option<&str>,
) -> Result<(Dispatch, LogGuard), DiagError> {
    let file =
        RotatingFile::open(&config.dir, LOG_FILE_NAME, config.rotation).map_err(|source| {
            DiagError::Log {
                path: config.dir.clone(),
                source,
            }
        })?;
    let (writer, worker) = NonBlockingBuilder::default()
        .lossy(false)
        .thread_name("rb-log")
        .finish(Redacting(file));
    let (filter, filter_handle) = reload::Layer::new(env_filter(config.level, rust_log));
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_thread_names(true)
        .with_writer(writer);
    let dispatch = Dispatch::new(Registry::default().with(filter).with(file_layer));
    let handle = LogHandle {
        filter: filter_handle,
        rust_log: rust_log.map(str::to_owned),
    };
    Ok((
        dispatch,
        LogGuard {
            _worker: worker,
            handle,
        },
    ))
}

/// Builds the logger with `RUST_LOG` from the environment, installs it for the whole process
/// and adds the panic hook. Call once, first thing in `main`, and keep the guard.
pub fn init(config: &LogConfig) -> Result<LogGuard, DiagError> {
    let rust_log = std::env::var("RUST_LOG").ok();
    let (dispatch, guard) = build(config, rust_log.as_deref())?;
    tracing::dispatcher::set_global_default(dispatch).map_err(|_| DiagError::AlreadyInstalled)?;
    crate::panic::install_hook();
    tracing::info!(
        target: target::CORE,
        version = env!("CARGO_PKG_VERSION"),
        dir = %config.dir.display(),
        "logging started"
    );
    Ok(guard)
}

/// Masks every line before it reaches the file. The fmt layer writes one event per `write`, so
/// a line is never split between two calls.
struct Redacting<W>(W);

impl<W: Write> Write for Redacting<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        self.0.write_all(crate::redact::redact(&text).as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[cfg(test)]
#[path = "logging_tests.rs"]
mod tests;
