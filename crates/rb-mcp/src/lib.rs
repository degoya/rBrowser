//! rBrowser's MCP server (RB-030-01): rBrowser fully controllable by AI agents.
//!
//! - [`http`]: Streamable HTTP on `127.0.0.1:<random port>`, stateless, every request with the
//!   bearer token ([`token`], OS randomness, constant-time compare), `Host` and `Origin` held to
//!   the loopback; `/health` answers a nonce with the token's HMAC proof.
//! - [`instance`]: the instance file per project (port, token, pid; the user's own) and the
//!   nonce probe that tells a live instance from a file left by a crash.
//! - [`stdio`]: `rbrowser mcp`, the stdio front end that forwards to the running instance, finds
//!   it again after a restart and answers with a clear message while there is none (K7).
//! - [`server`], [`tools`]: the core tools, unprefixed, each a thin layer over a command of the
//!   one command registry or a read of the host ([`Backend`], implemented by `crates/rbrowser`);
//!   every view tool takes `target`: `"all"`, a view id or a breakpoint id ([`target`]).
//! - [`coverage`]: every command's MCP entry resolves to a tool or a reason, every tool is
//!   described, typed and targeted (RB-030-08); `schema/mcp-tools.v1.json` for the reference.
//! - [`prompts`], [`resources`]: the workflows as MCP prompts and rBrowser's state as read-only
//!   MCP resources (RB-095-01), behind `mcp.prompts` and `mcp.resources`, with the tools' token
//!   and limits; `rbrowser://config` leaves every `x-protected` key out ([`protected_keys`]).
//!
//! MCP can never grant workspace trust, install or trust a plugin, or write an `x-protected`
//! setting: there is no tool for the first two, and `settings_set` writes as an MCP caller.

pub mod activity;
pub mod answer;
pub mod backend;
pub mod config_edit;
pub mod coverage;
pub mod http;
pub mod instance;
mod issues;
pub mod logs;
pub mod prompts;
pub mod protected_keys;
pub mod resources;
pub mod secret;
pub mod server;
pub mod settings_view;
pub mod settle;
pub mod stdio;
pub mod target;
#[doc(hidden)]
pub mod testing;
pub mod token;
pub mod tools;

pub use backend::{Backend, ConfigScope, McpOptions, SettingScope, ToolFailure};
pub use http::{HttpHandle, McpHttpConfig, serve_http};
pub use logs::{IssueEntry, LogKind, LogLimits, LogStore};
pub use server::RbMcpServer;
pub use token::McpToken;
pub use tools::plugins::PluginToolInfo;
