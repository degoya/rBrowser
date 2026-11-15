//! The component: `src/lib.rs` translated into the vocabulary of the contract
//! (`crates/rb-plugin-api/wit`, world `export-plugin`). Built only for `wasm32`; it holds no logic
//! of its own. The exporter calls no host interface, so the component imports none and is
//! granted no capability.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    path: "../../crates/rb-plugin-api/wit",
    world: "export-plugin",
});

use exports::rbrowser::plugin::exporter::{ExportFormat, ExportOutput, Guest};
use rb_plugin_api::{Failure, FailureKind};
use rbrowser::plugin::types;

struct Component;

fn to_wit(failure: Failure) -> types::Failure {
    let kind = match failure.kind {
        FailureKind::NotFound => types::FailureKind::NotFound,
        FailureKind::AccessDenied => types::FailureKind::AccessDenied,
        FailureKind::NotSupported => types::FailureKind::NotSupported,
        FailureKind::BadData => types::FailureKind::BadData,
        FailureKind::Cancelled => types::FailureKind::Cancelled,
        FailureKind::Io => types::FailureKind::Io,
        FailureKind::Limit => types::FailureKind::Limit,
        FailureKind::Other => types::FailureKind::Other,
    };
    types::Failure {
        kind,
        code: failure.code,
        params: failure.params.into_iter().collect(),
    }
}

impl Guest for Component {
    fn formats() -> Vec<ExportFormat> {
        use rb_plugin_api::export::Exporter as _;
        crate::RecorderExport
            .formats()
            .into_iter()
            .map(|format| ExportFormat {
                id: format.id,
                title: format.title,
                extension: format.extension,
            })
            .collect()
    }

    fn render(format: String, recording: String) -> Result<ExportOutput, types::Failure> {
        let output = crate::render(&format, &recording).map_err(to_wit)?;
        Ok(ExportOutput {
            file_name: output.file_name,
            text: output.text,
        })
    }
}

export!(Component);
