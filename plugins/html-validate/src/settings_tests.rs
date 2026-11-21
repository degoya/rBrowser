use rb_plugin_api::Severity;

use super::{Engine, Settings, Source, nu_endpoint};
use crate::Plan;
use crate::fake::FakeHost;

fn read(pairs: &[(&str, &str)]) -> Result<Settings, rb_plugin_api::Failure> {
    let mut host = FakeHost::default();
    for (key, value) in pairs {
        host = host.set(key, value);
    }
    Settings::read(&mut host)
}

/// The address in `pairs` as the Nu engine checks it when a run starts.
fn nu_plan(pairs: &[(&str, &str)]) -> Result<Plan, rb_plugin_api::Failure> {
    let settings = read(pairs)?;
    Plan::new(settings, Some(Engine::Nu), Vec::new())
}

fn reason_key(failure: &rb_plugin_api::Failure) -> Option<&str> {
    failure.param("key")
}

#[test]
fn the_defaults_are_offline_without_an_address() {
    let settings = read(&[]).expect("defaults");
    assert_eq!(settings.engine, Engine::Builtin);
    assert_eq!(settings.nu_address, None);
    assert!(settings.disabled_rules.is_empty());
    assert_eq!(settings.min_severity, Severity::Warning);
    assert_eq!(settings.source, Source::Raw);
}

#[test]
fn reads_every_key() {
    let settings = read(&[
        ("engine", "\"nu\""),
        ("nuAddress", "\"http://localhost:8888/\""),
        ("disabledRules", "[\"no-raw-characters\", \"nu-info\"]"),
        ("minSeverity", "\"error\""),
        ("source", "\"rendered\""),
    ])
    .expect("settings");
    assert_eq!(settings.source, Source::Rendered);
    assert_eq!(settings.engine, Engine::Nu);
    assert_eq!(
        settings.nu_address.as_deref(),
        Some("http://localhost:8888/")
    );
    assert_eq!(settings.disabled_rules, ["no-raw-characters", "nu-info"]);
    assert_eq!(settings.min_severity, Severity::Error);
}

#[test]
fn an_empty_or_blank_address_is_none() {
    for value in ["\"\"", "\"   \"", "null"] {
        let settings = read(&[("nuAddress", value)]).expect(value);
        assert_eq!(settings.nu_address, None, "{value}");
    }
}

#[test]
fn refuses_an_address_that_is_no_http_url() {
    for address in [
        "ftp://localhost/",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "localhost:8888",
        "http://",
        "http://user:secret@localhost:8888/",
        "http://localhost:8888/?out=xml",
        "http://localhost:8888/#top",
        "http://local host/",
    ] {
        let value = serde_json::to_string(address).expect("json");
        let failure = nu_plan(&[("nuAddress", &value)]).expect_err(address);
        assert_eq!(reason_key(&failure), Some("nuAddress"), "{address}");
    }
}

#[test]
fn a_bad_address_does_not_stop_the_builtin_engine() {
    let pairs = [
        ("engine", "\"builtin\""),
        ("nuAddress", "\"localhost:8888\""),
    ];
    let settings = read(&pairs).expect("the address is the Nu engine's");
    assert_eq!(settings.engine, Engine::Builtin);
    Plan::new(settings, None, Vec::new()).expect("builtin plan");
    let failure = nu_plan(&pairs).expect_err("the Nu engine checks it");
    assert_eq!(reason_key(&failure), Some("nuAddress"));
}

#[test]
fn the_refusal_of_an_address_never_repeats_its_credentials() {
    let failure = nu_plan(&[("nuAddress", "\"https://user:secret@example.test/\"")])
        .expect_err("credentials");
    assert!(
        failure
            .params
            .values()
            .all(|value| !value.contains("secret"))
    );
}

#[test]
fn refuses_an_unknown_engine_and_severity() {
    let failure = read(&[("engine", "\"vnu\"")]).expect_err("engine");
    assert_eq!(reason_key(&failure), Some("engine"));
    let failure = read(&[("minSeverity", "\"fatal\"")]).expect_err("severity");
    assert_eq!(reason_key(&failure), Some("minSeverity"));
}

#[test]
fn refuses_a_malformed_or_overlong_rule_list() {
    let failure = read(&[("disabledRules", "[\"No Such\"]")]).expect_err("id");
    assert_eq!(reason_key(&failure), Some("disabledRules"));
    let many: Vec<String> = (0..65).map(|n| format!("rule-{n}")).collect();
    let value = serde_json::to_string(&many).expect("json");
    let failure = read(&[("disabledRules", &value)]).expect_err("many");
    assert_eq!(reason_key(&failure), Some("disabledRules"));
}

#[test]
fn the_endpoint_adds_the_slash_and_the_json_output() {
    assert_eq!(
        nu_endpoint("http://localhost:8888"),
        "http://localhost:8888/?out=json"
    );
    assert_eq!(
        nu_endpoint("http://localhost:8888/"),
        "http://localhost:8888/?out=json"
    );
    assert_eq!(
        nu_endpoint("https://validator.w3.org/nu"),
        "https://validator.w3.org/nu/?out=json"
    );
}

#[test]
fn severity_order_keeps_what_is_at_or_above_the_lowest() {
    let settings = read(&[("minSeverity", "\"warning\"")]).expect("settings");
    assert!(settings.keeps(Severity::Error));
    assert!(settings.keeps(Severity::Warning));
    assert!(!settings.keeps(Severity::Info));
}

#[test]
fn refuses_an_unknown_source() {
    let failure = read(&[("source", "\"served\"")]).expect_err("source");
    assert_eq!(reason_key(&failure), Some("source"));
    assert_eq!(Source::Raw.as_str(), "raw");
    assert_eq!(Source::Rendered.as_str(), "rendered");
}

#[test]
fn the_project_rule_file_takes_precedence_over_the_rules_off() {
    let rules = crate::project::parse(br#"{"rules": {"no-dup-id": "warn", "close-order": "off"}}"#)
        .expect("rules");
    let settings = Settings {
        disabled_rules: vec!["no-dup-id".to_owned(), "void-content".to_owned()],
        project: crate::project::ProjectRules::applied(rules),
        ..Settings::default()
    };
    // Switched off in the settings, on in the file: on.
    assert!(!settings.disabled("no-dup-id"));
    // Off in the file only: off.
    assert!(settings.disabled("close-order"));
    // The file is silent: the setting decides.
    assert!(settings.disabled("void-content"));
    assert!(!settings.disabled("element-name"));
}
