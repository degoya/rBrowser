//! WSL for rBrowser on Windows (RB-030-04): projects live in a WSL distribution, rBrowser runs
//! on Windows.
//!
//! - [`path`]: UNC (`\\wsl.localhost\<distro>\...`, `\\wsl$\...`) and drive (`C:\` ->
//!   `/mnt/c`) paths in both directions.
//! - [`distro`], [`query`]: the installed distributions from `wsl.exe --list --verbose` (UTF-16
//!   output, [`decode`]), the default, running or stopped.
//! - [`netmode`]: the networking mode (`mirrored` or NAT) per distribution, from `wslinfo
//!   --networking-mode`, `.wslconfig` and the override setting, cached.
//! - [`protocol`], [`serve`]: the versioned line-JSON stdio protocol between rBrowser and its
//!   helper `rbrowser-wsl` (`src/bin/rbrowser-wsl.rs`), with a handshake of version and nonce.
//! - [`helper`], [`supervisor`]: starting the helper through `wsl.exe -d <distro> -e <path>`,
//!   restarting it after a crash, stopping it with the app.
//! - [`settings`], [`info`]: the `wsl.*` settings and the answer of the MCP tool `wsl_info`.
//! - [`watch`]: the file watch for hot reload (RB-030-05): ignore rules, debounce, the tree of
//!   watches; the helper's `watch` loop, and the same parts for the app's own watcher.
//! - [`cdp_proxy`]: the CDP tunnel for NAT mode (RB-030-06): the helper's `cdp-proxy` listener
//!   in the distribution, multiplexed over stdio to the app's token-protected proxy.
//!
//! Everything but the process calls is pure and tested on every platform; the process calls
//! take the program as a parameter and start it without a console window.

pub mod cdp_proxy;
pub mod decode;
pub mod distro;
mod error;
pub mod helper;
pub mod info;
pub mod netmode;
pub mod path;
pub mod protocol;
pub mod query;
pub mod serve;
pub mod settings;
pub mod supervisor;
mod supervisor_io;
pub mod watch;

pub use error::WslError;
