//! Generated bindings for `rbrowser:plugin@0.1.0`: the records of every interface.
//!
//! As rCommander's `crates/rc-plugin-host/src/bindings.rs` generates them: one `bindgen!` per
//! world, async imports and exports, and the later worlds map the shared imports onto the first
//! world's modules (`with: { ... }`), so a `failure` or a `finding` is one type across every
//! world. The host links by hand (`link.rs`) - only what the manifest grants - and finds the
//! exports by name (`plugin/exports.rs`), so the generated linker and world structs stay unused.

wasmtime::component::bindgen!({
    path: "../rb-plugin-api/wit",
    world: "feature-plugin",
    imports: { default: async },
    exports: { default: async },
});

/// The check world's own interface: the `check` export.
pub(crate) mod check_world {
    wasmtime::component::bindgen!({
        path: "../rb-plugin-api/wit",
        world: "check-plugin",
        imports: { default: async },
        exports: { default: async },
        with: {
            "rbrowser:plugin/types": crate::bindings::rbrowser::plugin::types,
            "rbrowser:plugin/host": crate::bindings::rbrowser::plugin::host,
            "rbrowser:plugin/settings": crate::bindings::rbrowser::plugin::settings,
            "rbrowser:plugin/views": crate::bindings::rbrowser::plugin::views,
            "rbrowser:plugin/evaluate": crate::bindings::rbrowser::plugin::evaluate,
            "rbrowser:plugin/cdp": crate::bindings::rbrowser::plugin::cdp,
            "rbrowser:plugin/http": crate::bindings::rbrowser::plugin::http,
            "rbrowser:plugin/project": crate::bindings::rbrowser::plugin::project,
            "rbrowser:plugin/project-range": crate::bindings::rbrowser::plugin::project_range,
            "rbrowser:plugin/project-write": crate::bindings::rbrowser::plugin::project_write,
            "rbrowser:plugin/out": crate::bindings::rbrowser::plugin::out,
            "rbrowser:plugin/out-stream": crate::bindings::rbrowser::plugin::out_stream,
            "rbrowser:plugin/findings": crate::bindings::rbrowser::plugin::findings,
            "rbrowser:plugin/sidecar": crate::bindings::rbrowser::plugin::sidecar,
        },
    });
}

/// The analyzer world's own interface: the `analyzer` export.
pub(crate) mod analyzer_world {
    wasmtime::component::bindgen!({
        path: "../rb-plugin-api/wit",
        world: "analyzer-plugin",
        imports: { default: async },
        exports: { default: async },
        with: {
            "rbrowser:plugin/types": crate::bindings::rbrowser::plugin::types,
            "rbrowser:plugin/host": crate::bindings::rbrowser::plugin::host,
            "rbrowser:plugin/settings": crate::bindings::rbrowser::plugin::settings,
            "rbrowser:plugin/views": crate::bindings::rbrowser::plugin::views,
            "rbrowser:plugin/evaluate": crate::bindings::rbrowser::plugin::evaluate,
            "rbrowser:plugin/cdp": crate::bindings::rbrowser::plugin::cdp,
            "rbrowser:plugin/http": crate::bindings::rbrowser::plugin::http,
            "rbrowser:plugin/project": crate::bindings::rbrowser::plugin::project,
            "rbrowser:plugin/project-range": crate::bindings::rbrowser::plugin::project_range,
            "rbrowser:plugin/project-write": crate::bindings::rbrowser::plugin::project_write,
            "rbrowser:plugin/out": crate::bindings::rbrowser::plugin::out,
            "rbrowser:plugin/out-stream": crate::bindings::rbrowser::plugin::out_stream,
            "rbrowser:plugin/findings": crate::bindings::rbrowser::plugin::findings,
            "rbrowser:plugin/sidecar": crate::bindings::rbrowser::plugin::sidecar,
        },
    });
}

/// The export world's own interface: the `exporter` export.
pub(crate) mod export_world {
    wasmtime::component::bindgen!({
        path: "../rb-plugin-api/wit",
        world: "export-plugin",
        imports: { default: async },
        exports: { default: async },
        with: {
            "rbrowser:plugin/types": crate::bindings::rbrowser::plugin::types,
            "rbrowser:plugin/host": crate::bindings::rbrowser::plugin::host,
            "rbrowser:plugin/settings": crate::bindings::rbrowser::plugin::settings,
            "rbrowser:plugin/views": crate::bindings::rbrowser::plugin::views,
            "rbrowser:plugin/evaluate": crate::bindings::rbrowser::plugin::evaluate,
            "rbrowser:plugin/cdp": crate::bindings::rbrowser::plugin::cdp,
            "rbrowser:plugin/http": crate::bindings::rbrowser::plugin::http,
            "rbrowser:plugin/project": crate::bindings::rbrowser::plugin::project,
            "rbrowser:plugin/project-range": crate::bindings::rbrowser::plugin::project_range,
            "rbrowser:plugin/project-write": crate::bindings::rbrowser::plugin::project_write,
            "rbrowser:plugin/out": crate::bindings::rbrowser::plugin::out,
            "rbrowser:plugin/out-stream": crate::bindings::rbrowser::plugin::out_stream,
            "rbrowser:plugin/findings": crate::bindings::rbrowser::plugin::findings,
            "rbrowser:plugin/sidecar": crate::bindings::rbrowser::plugin::sidecar,
        },
    });
}
