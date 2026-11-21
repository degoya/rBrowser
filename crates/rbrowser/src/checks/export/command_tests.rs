use rb_commands::{Binding, Group, McpCoverage};
use rb_plugin_types::{CheckFailure, CheckSkip, SKIP_ENGINE};

use super::super::testing::fixture;
use super::*;

#[test]
fn the_export_is_a_checks_command_with_its_tool_and_chord() {
    let defs = definitions().expect("valid");
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].id.as_str(), "checks.export");
    assert_eq!(defs[0].title_key, "commands.checks.export");
    assert_eq!(defs[0].group, Group::Checks);
    assert_eq!(
        defs[0].default_bindings,
        vec!["Ctrl+Shift+Space K B".parse::<Binding>().expect("binding")]
    );
    assert_eq!(
        defs[0].mcp,
        McpCoverage::Covered("checks_export".to_owned())
    );
}

#[test]
fn the_arguments_are_a_format_and_a_path() {
    assert_eq!(parse_args(&Value::Null).expect("none"), (None, None));
    assert_eq!(
        parse_args(&json!({ "format": "junit", "path": "ci/x.xml" })).expect("both"),
        (Some(ExportFormat::Junit), Some("ci/x.xml".to_owned()))
    );
    assert!(matches!(
        parse_args(&json!({ "format": "pdf" })),
        Err(ExportError::Invalid(_))
    ));
    assert!(matches!(
        parse_args(&json!({ "target": "all" })),
        Err(ExportError::Invalid(_))
    ));
    assert!(matches!(
        parse_args(&json!({ "path": 3 })),
        Err(ExportError::Invalid(_))
    ));
}

#[test]
fn a_refused_path_is_an_invalid_argument() {
    let error: CommandError = ExportError::Invalid("no".to_owned()).into();
    assert!(matches!(error, CommandError::InvalidArgs(_)));
    let error: CommandError = ExportError::Failed("no".to_owned()).into();
    assert!(matches!(error, CommandError::Failed(_)));
}

#[test]
fn the_core_texts_of_failures_and_skips_join_the_messages() {
    let mut file = fixture();
    file.failures.push(CheckFailure {
        plugin: "seo".to_owned(),
        view: Some(1),
        code: Some("plugins.error.crashed".to_owned()),
        params: Default::default(),
    });
    file.skipped.push(CheckSkip {
        plugin: "seo".to_owned(),
        view: Some(2),
        reason: SKIP_ENGINE.to_owned(),
        engine: Some("webkit".to_owned()),
    });
    let messages = BTreeMap::from([("plugin.seo.x".to_owned(), "X".to_owned())]);
    let all = with_core_texts(&file, &messages);
    assert_eq!(all.get("plugin.seo.x").map(String::as_str), Some("X"));
    assert!(all.contains_key("plugins.error.crashed"));
    assert!(all.contains_key("checks.skip.engine"));
}
