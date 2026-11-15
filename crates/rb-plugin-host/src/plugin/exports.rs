//! Where the exports of each declared kind are in one component, found before instantiating, as
//! rCommander's `*/guest.rs` find them (`get_export_index` under the versioned interface name).

use rb_plugin_api::versioned;
use rb_plugin_types::PluginKind;
use wasmtime::component::{Component, ComponentExportIndex};

/// The functions of `interface feature`.
#[derive(Clone, Copy)]
pub(crate) struct FeatureExports {
    pub(crate) activate: ComponentExportIndex,
    pub(crate) run_command: ComponentExportIndex,
    pub(crate) call_tool: ComponentExportIndex,
}

/// The functions of `interface check`.
#[derive(Clone, Copy)]
pub(crate) struct CheckExports {
    pub(crate) describe: ComponentExportIndex,
    pub(crate) run: ComponentExportIndex,
}

/// The function of `interface analyzer`.
#[derive(Clone, Copy)]
pub(crate) struct AnalyzerExports {
    pub(crate) analyze: ComponentExportIndex,
}

/// The functions of `interface exporter`.
#[derive(Clone, Copy)]
pub(crate) struct ExporterExports {
    pub(crate) formats: ComponentExportIndex,
    pub(crate) render: ComponentExportIndex,
}

/// The exports of every kind the manifest declares; `None` for a kind it does not.
#[derive(Clone, Copy, Default)]
pub(crate) struct Exports {
    pub(crate) feature: Option<FeatureExports>,
    pub(crate) check: Option<CheckExports>,
    pub(crate) analyzer: Option<AnalyzerExports>,
    pub(crate) exporter: Option<ExporterExports>,
}

impl Exports {
    /// The exports of `kinds` in `component`; `Err(kind)` for the first kind whose interface or
    /// one of its functions the component does not export.
    pub(crate) fn find(component: &Component, kinds: &[PluginKind]) -> Result<Self, PluginKind> {
        let mut exports = Self::default();
        for kind in kinds {
            let interface = component
                .get_export_index(None, versioned(kind.export()))
                .ok_or(*kind)?;
            let function = |name: &str| {
                component
                    .get_export_index(Some(&interface), name)
                    .ok_or(*kind)
            };
            match kind {
                PluginKind::Feature => {
                    exports.feature = Some(FeatureExports {
                        activate: function("activate")?,
                        run_command: function("run-command")?,
                        call_tool: function("call-tool")?,
                    });
                }
                PluginKind::Check => {
                    exports.check = Some(CheckExports {
                        describe: function("describe")?,
                        run: function("run")?,
                    });
                }
                PluginKind::Analyzer => {
                    exports.analyzer = Some(AnalyzerExports {
                        analyze: function("analyze")?,
                    });
                }
                PluginKind::Export => {
                    exports.exporter = Some(ExporterExports {
                        formats: function("formats")?,
                        render: function("render")?,
                    });
                }
            }
        }
        Ok(exports)
    }
}
