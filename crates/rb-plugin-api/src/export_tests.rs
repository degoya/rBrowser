use super::{Exporter, valid_file_name};
use crate::test_support::MemoryHost;
use crate::{ExportFormat, ExportOutput, Failure, FailureKind, PluginHost};

/// Writes every recorded URL as a `page.goto` line.
struct GotoExporter;

impl Exporter for GotoExporter {
    fn formats(&self) -> Vec<ExportFormat> {
        vec![ExportFormat {
            id: "playwright-ts".to_owned(),
            title: "plugin.goto.formats.playwright".to_owned(),
            extension: "ts".to_owned(),
        }]
    }

    fn render(
        &mut self,
        _host: &mut dyn PluginHost,
        format: &str,
        recording: &str,
    ) -> Result<ExportOutput, Failure> {
        if format != "playwright-ts" {
            return Err(Failure::coded(
                FailureKind::NotSupported,
                "plugin.goto.unknown_format",
            ));
        }
        let text = recording
            .lines()
            .map(|url| format!("await page.goto({url:?});\n"))
            .collect();
        Ok(ExportOutput {
            file_name: "flow.spec.ts".to_owned(),
            text,
        })
    }
}

#[test]
fn an_exporter_renders_natively() {
    let mut host = MemoryHost::new();
    let output = GotoExporter
        .render(&mut host, "playwright-ts", "https://example.org/")
        .expect("rendered");
    assert_eq!(output.text, "await page.goto(\"https://example.org/\");\n");
    assert!(valid_file_name(&output.file_name));
    let refused = GotoExporter
        .render(&mut host, "cypress", "")
        .expect_err("unknown format");
    assert_eq!(refused.code(), Some("plugin.goto.unknown_format"));
    assert_eq!(GotoExporter.formats()[0].extension, "ts");
}

#[test]
fn an_export_file_name_has_no_folder() {
    assert!(valid_file_name("flow.spec.ts"));
    for name in ["", "a/b.ts", "../b.ts", "CON", "x.ts."] {
        assert!(!valid_file_name(name), "{name}");
    }
}
