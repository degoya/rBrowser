//! rBrowser's plugin contract (RB-010-03): the WIT package `rbrowser:plugin@0.1.0` and the native
//! side of each world, so a plugin's logic is tested without a WebAssembly toolchain.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-api` (itself from rDownloader's
//! `rd-plugin-api`): one WIT world per plugin kind and a native trait per world. rBrowser's four
//! worlds are `feature-plugin` ([`feature`]), `check-plugin` ([`check`]), `analyzer-plugin`
//! ([`analyzer`]) and `export-plugin` ([`export`]); every world may import the same host
//! interfaces, which the native side gathers in one trait, [`PluginHost`], and the host grants by
//! capability.
//!
//! The crate builds for `wasm32-unknown-unknown` as well: a plugin's guest half translates the
//! generated WIT bindings into the traits here and nothing else - its host and conversions are
//! one macro, [`guest_host!`] (RB-094-11).

#![warn(unreachable_pub)]

pub mod analyzer;
pub mod check;
pub mod export;
pub mod feature;
mod guest;
mod host;

/// The in-memory host a plugin's native tests run against; never part of a component or the host.
#[cfg(all(any(test, feature = "test-support"), not(target_arch = "wasm32")))]
pub mod test_support;

pub use host::PluginHost;
pub use rb_plugin_types::{
    Capability, CheckContext, ExportFormat, ExportOutput, Failure, FailureKind, Finding,
    HttpRequest, HttpResponse, MessageParams, PluginKind, Proposal, RULE_LEVELS_SETTING, RuleInfo,
    Severity, ViewId, ViewInfo, codes, html, paths, rules,
};

/// The WIT package, exactly as the host's bindings and every SDK template are generated from.
pub const WIT: &str = include_str!("../wit/rbrowser-plugin.wit");

/// The version of `rbrowser:plugin` this contract is; a manifest's `api_version`.
pub const API_VERSION: &str = "0.1.0";

/// The package name every interface of the contract is imported and exported under.
pub const PACKAGE: &str = "rbrowser:plugin";

/// The most bytes one `project.read` answers or one `out.write` takes.
pub const MAX_IO_BYTES: usize = 8 * 1024 * 1024;

/// The most characters of one JSON answer (`run-command`, `call-tool`, `evaluate`, `cdp.send`,
/// `sidecar.call`) the host accepts.
pub const MAX_JSON_CHARS: usize = 4 * 1024 * 1024;

/// `interface` of the contract under its version, as a component names its import:
/// `rbrowser:plugin/http@0.1.0`.
#[must_use]
pub fn versioned(interface: &str) -> String {
    format!("{PACKAGE}/{interface}@{API_VERSION}")
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod contract_tests;
