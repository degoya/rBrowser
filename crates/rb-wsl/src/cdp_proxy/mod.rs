//! The CDP tunnel for WSL in NAT mode (RB-030-06). There `127.0.0.1` inside the distribution
//! is not Windows' own, so a client in WSL (Playwright, an agent) cannot reach the proxy
//! rBrowser opens on Windows. The helper's `cdp-proxy` subcommand listens on `127.0.0.1` in the
//! distribution instead and carries every connection it accepts over its stdio to the app,
//! which opens the matching connection to its proxy:
//!
//! ```text
//! WSL client -> 127.0.0.1:<port> (rbrowser-wsl cdp-proxy) == stdio ==> wsl.exe -> rBrowser
//!   -> 127.0.0.1:<proxy port> (the token-protected CDP proxy)
//! ```
//!
//! The tunnel carries bytes, not CDP: the token is checked by the proxy as for any client, so
//! the tunnel opens nothing that was not open before (threat model T13). Many connections share
//! the one stdio pair ([`frame`]: `open`, `data`, `close` per connection id; [`mux`]).
//!
//! - [`frame`]: the tunnel's line-JSON messages after the handshake;
//! - [`mux`]: both ends of the multiplex, over any line reader and writer;
//! - [`helper`]: the helper's side - handshake, listen, `listening`, multiplex;
//! - [`app`]: the app's side - start the helper through `wsl.exe`, handshake, multiplex.

pub mod app;
pub mod frame;
pub mod helper;
pub mod mux;
