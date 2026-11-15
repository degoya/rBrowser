use crate::ProjectConfig;

use super::*;

#[test]
fn the_section_reads_its_keys_and_defaults_the_rest() {
    let config: ProjectConfig = serde_json::from_str(
        r#"{ "version": 1, "envCompare": { "environments": ["local", "staging"],
             "threshold": 0.2, "maxDiffPercent": 0.5, "parallel": 2 } }"#,
    )
    .expect("parses");
    let section = &config.env_compare;
    assert_eq!(section.environments, ["local", "staging"]);
    assert_eq!(section.threshold, Some(0.2));
    assert!((section.max_diff_percent - 0.5).abs() < f64::EPSILON);
    assert_eq!(section.parallel, 2);
    assert!(section.breakpoints.is_empty());
    assert!(section.full_page, "defaults to true");
    assert_eq!(section.wait_ms, 0);
}

#[test]
fn a_default_section_is_not_written() {
    let config: ProjectConfig = serde_json::from_str(r#"{ "version": 1 }"#).expect("parses");
    assert_eq!(config.env_compare, EnvCompareConfig::default());
    assert_eq!(
        config.env_compare.threshold, None,
        "visual.threshold applies"
    );
    let written = serde_json::to_value(&config).expect("serializes");
    assert!(written.get("envCompare").is_none(), "{written}");
}
