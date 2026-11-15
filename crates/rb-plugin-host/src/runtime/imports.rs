//! Which WIT interfaces a component may import: `host` and `types` always, `settings` when the
//! manifest declares a settings schema, and one interface per granted capability - each under
//! the contract version the host runs. Everything else - WASI, another package, another version
//! - is refused when the component is compiled.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host/src/runtime/imports.rs` (one grant,
//! one interface): rBrowser's grants are its capabilities, not its plugin kinds.

use rb_plugin_api::versioned;
use rb_plugin_types::{Capability, Failure, FailureKind, codes};

use crate::{failure, manifest::PluginManifest};

/// Interfaces every plugin may import: the log, a clock, the shared types.
const BASE: [&str; 2] = ["host", "types"];

/// The interfaces one manifest permits, each with its version: `rbrowser:plugin/http@0.1.0`.
pub(crate) fn allowed_imports(manifest: &PluginManifest) -> Vec<String> {
    let mut allowed: Vec<&str> = BASE.to_vec();
    if manifest.settings {
        allowed.push("settings");
    }
    allowed.extend(
        manifest
            .grants
            .granted
            .iter()
            .filter_map(|capability| capability.interface()),
    );
    allowed.into_iter().map(versioned).collect()
}

/// Why the import `name` is refused: the capability that would grant it, or the import itself.
pub(crate) fn refused_import(name: &str) -> Failure {
    let shown = failure::readable(name, 128);
    let interface = name
        .strip_prefix(&format!("{}/", rb_plugin_api::PACKAGE))
        .and_then(|rest| rest.strip_suffix(&format!("@{}", rb_plugin_api::API_VERSION)));
    match interface.and_then(Capability::for_interface) {
        Some(capability) => capability.denied().with_param("import", shown),
        None => {
            failure::coded(FailureKind::AccessDenied, codes::DENIED).with_param("import", shown)
        }
    }
}
