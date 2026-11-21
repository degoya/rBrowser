use crate::ProjectConfig;

use super::*;

#[test]
fn the_section_reads_its_keys_and_defaults_the_rest() {
    let config: ProjectConfig = serde_json::from_str(
        r#"{ "version": 1, "engineCompare": { "threshold": 0.3, "maxDiffPercent": 1.5 } }"#,
    )
    .expect("parses");
    let section = &config.engine_compare;
    assert_eq!(section.threshold, Some(0.3));
    assert!((section.max_diff_percent - 1.5).abs() < f64::EPSILON);
    assert_eq!(
        section.wait_ms, DEFAULT_WAIT_MS,
        "the settle pause by default"
    );
}

#[test]
fn a_default_section_is_not_written() {
    let config: ProjectConfig = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(config.engine_compare, EngineCompareConfig::default());
    assert_eq!(
        config.engine_compare.threshold, None,
        "visual.threshold applies"
    );
    assert!(config.engine_compare.max_diff_percent.abs() < f64::EPSILON);
    let written = serde_json::to_value(&config).expect("serializes");
    assert!(written.get("engineCompare").is_none(), "{written}");
}
