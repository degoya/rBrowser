use crate::ProjectConfig;

use super::*;

#[test]
fn the_defaults_take_every_part_and_open_no_editor() {
    let config: ProjectConfig = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(config.report, BugReportConfig::default());
    assert_eq!(config.report.include, ReportPart::ALL);
    assert!(!config.report.annotate);
    let written = serde_json::to_value(&config).expect("serializes");
    assert!(
        written.get("report").is_none(),
        "a default section is not written: {written}"
    );
}

#[test]
fn the_section_reads_its_parts_and_the_editor() {
    let config: ProjectConfig = serde_json::from_str(
        r#"{ "version": 1, "report": { "include": ["screenshots", "console"], "annotate": true } }"#,
    )
    .expect("parses");
    assert_eq!(
        config.report.include,
        [ReportPart::Screenshots, ReportPart::Console]
    );
    assert!(config.report.annotate);
    assert!(config.report.has(ReportPart::Console));
    assert!(!config.report.has(ReportPart::Network));
}

#[test]
fn an_unknown_part_is_refused() {
    let parsed = serde_json::from_str::<ProjectConfig>(
        r#"{ "version": 1, "report": { "include": ["cookies"] } }"#,
    );
    assert!(parsed.is_err(), "cookies are never a part of the report");
}

#[test]
fn the_parts_are_camel_case_on_disk() {
    let names: Vec<String> = ReportPart::ALL
        .iter()
        .map(|part| serde_json::to_value(part).expect("serializes"))
        .map(|value| value.as_str().expect("a string").to_owned())
        .collect();
    assert_eq!(
        names,
        [
            "screenshots",
            "overview",
            "console",
            "network",
            "environment",
            "emulation",
            "version"
        ]
    );
}
