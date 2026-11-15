use rb_plugin_types::{ExportOutput, Finding, Proposal, RuleInfo, Severity, codes};

use super::{export_output, findings, json, proposals, rules};

const PREFIX: &str = "plugin.sample.";

fn finding(message: &str) -> Finding {
    Finding {
        rule: "title.missing".to_owned(),
        severity: Severity::Warning,
        message: message.to_owned(),
        ..Finding::default()
    }
}

fn rule(id: &str) -> RuleInfo {
    RuleInfo {
        id: id.to_owned(),
        title: "plugin.sample.rules.title".to_owned(),
        severity: Severity::Error,
        shared_id: None,
        help_url: None,
    }
}

#[test]
fn a_command_answer_is_json() {
    assert_eq!(
        json("{\"ok\":true}".to_owned()).expect("json"),
        "{\"ok\":true}"
    );
    let refused = json("not json".to_owned()).expect_err("not json");
    assert_eq!(refused.code(), Some(codes::BAD_OUTPUT));
    assert_eq!(refused.param("what"), Some("answer_not_json"));
}

#[test]
fn a_finding_speaks_in_the_plugins_own_keys() {
    assert!(findings(vec![finding("plugin.sample.findings.title")], PREFIX).is_ok());
    for message in ["toolbar.reload", "plugin.other.findings.title", "free text"] {
        assert!(
            findings(vec![finding(message)], PREFIX).is_err(),
            "{message}"
        );
    }
}

#[test]
fn a_proposal_names_a_pointer_and_a_json_value() {
    let proposal = |pointer: &str, value: &str| Proposal {
        pointer: pointer.to_owned(),
        value: value.to_owned(),
        reason: "plugin.sample.reason.width".to_owned(),
        params: Vec::new(),
    };
    assert!(proposals(vec![proposal("/breakpoints/0/width", "375")], PREFIX).is_ok());
    assert!(proposals(vec![proposal("breakpoints", "375")], PREFIX).is_err());
    assert!(proposals(vec![proposal("/breakpoints/0/width", "wide")], PREFIX).is_err());
}

#[test]
fn an_export_names_one_file_without_folders() {
    let output = |name: &str| ExportOutput {
        file_name: name.to_owned(),
        text: String::new(),
    };
    assert!(export_output(output("flow.spec.ts")).is_ok());
    assert!(export_output(output("../flow.spec.ts")).is_err());
    assert!(export_output(output("a/flow.spec.ts")).is_err());
}

#[test]
fn a_finding_names_a_rule_id_and_carries_json_data() {
    let mut good = finding("plugin.sample.findings.title");
    good.data = Some("{\"width\":3}".to_owned());
    good.path = Some("html > body".to_owned());
    assert!(findings(vec![good.clone()], PREFIX).is_ok());

    let mut bad_rule = good.clone();
    bad_rule.rule = "has space".to_owned();
    let mut bad_data = good.clone();
    bad_data.data = Some("not json".to_owned());
    let mut long_data = good;
    long_data.data = Some(format!("\"{}\"", "x".repeat(70 * 1024)));
    for broken in [bad_rule, bad_data, long_data] {
        let refused = findings(vec![broken], PREFIX).expect_err("refused");
        assert_eq!(refused.param("what"), Some("finding"));
    }
}

#[test]
fn rules_are_unique_own_keys_with_web_help() {
    let mut helped = rule("images.alt");
    helped.help_url = Some("https://help.test/alt".to_owned());
    helped.shared_id = Some("img-alt-missing".to_owned());
    assert!(rules(vec![helped, rule("title")], PREFIX).is_ok());

    let twice = vec![rule("title"), rule("title")];
    let mut foreign = rule("title");
    foreign.title = "plugin.other.rules.title".to_owned();
    let mut script = rule("title");
    script.help_url = Some("javascript:alert(1)".to_owned());
    let mut shared = rule("title");
    shared.shared_id = Some("no spaces allowed".to_owned());
    for broken in [twice, vec![foreign], vec![script], vec![shared]] {
        let refused = rules(broken, PREFIX).expect_err("refused");
        assert_eq!(refused.param("what"), Some("rule_info"));
    }
}
