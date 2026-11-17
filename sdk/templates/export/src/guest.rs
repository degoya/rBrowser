//! The component: `src/lib.rs` translated into the vocabulary of `wit/rbrowser-plugin.wit`. Built
//! only for `wasm32`; it holds no logic of its own, so it needs no test of its own.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    path: "wit",
    world: "export-plugin",
});

use exports::rbrowser::plugin::exporter::{ExportFormat, ExportOutput, Guest};
use rbrowser::plugin::types::{Failure, FailureKind};

struct Component;

impl Guest for Component {
    fn formats() -> Vec<ExportFormat> {
        vec![ExportFormat {
            id: crate::PLAYWRIGHT.to_owned(),
            title: "plugin.example-export.formats.playwright".to_owned(),
            extension: "ts".to_owned(),
        }]
    }

    fn render(format: String, recording: String) -> Result<ExportOutput, Failure> {
        if format != crate::PLAYWRIGHT {
            return Err(Failure {
                kind: FailureKind::NotSupported,
                code: Some("plugins.error.unknown_contribution".to_owned()),
                params: vec![("name".to_owned(), format)],
            });
        }
        let text = crate::render(&recording).map_err(|_| Failure {
            kind: FailureKind::BadData,
            code: None,
            params: Vec::new(),
        })?;
        Ok(ExportOutput {
            file_name: "recorded.spec.ts".to_owned(),
            text,
        })
    }
}

export!(Component);
