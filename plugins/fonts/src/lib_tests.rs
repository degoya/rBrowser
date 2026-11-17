use rb_plugin_api::check::Check;
use rb_plugin_api::feature::Feature;

use super::*;
use crate::fake::FakeHost;

#[test]
fn the_panel_command_opens_the_panel() {
    let mut host = FakeHost::default();
    let answer = Fonts.run_command(&mut host, "panel", "{}").expect("panel");
    assert_eq!(answer, r#"{"$open":{"panel":"fonts"}}"#);
}

#[test]
fn unknown_commands_and_tools_are_refused() {
    let mut host = FakeHost::default();
    for failure in [
        Fonts
            .run_command(&mut host, "nope", "")
            .expect_err("command"),
        Fonts.call_tool(&mut host, "nope", "").expect_err("tool"),
    ] {
        assert_eq!(failure.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    }
    let failure = Fonts
        .call_tool(&mut host, "list", "[1]")
        .expect_err("arguments");
    assert_eq!(failure.code(), Some(codes::INVALID));
}

#[test]
fn describe_names_every_rule_with_its_title_key() {
    let rules = Fonts.describe();
    assert_eq!(rules.len(), rules::RULES.len());
    assert!(
        rules
            .iter()
            .all(|rule| rule.title == key(&format!("rules.{}", rule.id)))
    );
    assert!(rules.iter().all(|rule| rule.id.len() <= 64));
}
