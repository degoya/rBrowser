use std::collections::BTreeMap;

use super::*;

struct Fake {
    vars: BTreeMap<&'static str, Value>,
}

impl Lookup for Fake {
    fn variable(&self, name: &str) -> Option<Value> {
        self.vars.get(name).cloned()
    }

    fn dynamic(&self, name: &str, args: &str) -> Option<Value> {
        match (name, args) {
            ("$processEnv", "RB_SECRET_TOKEN") => Some(Value::secret("s3cr3t")),
            ("$timestamp", "") => Some(Value::plain("1700000000")),
            _ => None,
        }
    }
}

fn fake() -> Fake {
    Fake {
        vars: BTreeMap::from([
            ("baseUrl", Value::plain("https://api.example.com")),
            ("users", Value::plain("{{baseUrl}}/users")),
            ("token", Value::secret("tok-123")),
            ("loop", Value::plain("{{loop}}")),
        ]),
    }
}

#[test]
fn variables_and_dynamic_ones_are_put_in() {
    let resolved = substitute(
        "{{ baseUrl }}/a?t={{$timestamp}}&k={{$processEnv RB_SECRET_TOKEN}}",
        &fake(),
    )
    .expect("resolves");
    assert_eq!(
        resolved.text,
        "https://api.example.com/a?t=1700000000&k=s3cr3t"
    );
    assert_eq!(resolved.secrets, ["s3cr3t"]);
}

#[test]
fn a_value_with_variables_is_resolved_again() {
    let resolved = substitute("{{users}}/1", &fake()).expect("resolves");
    assert_eq!(resolved.text, "https://api.example.com/users/1");
}

#[test]
fn a_value_that_names_itself_stops_at_the_depth() {
    let resolved = substitute("{{loop}}", &fake()).expect("resolves");
    assert_eq!(resolved.text, "{{loop}}");
}

#[test]
fn secrets_are_collected_once() {
    let resolved = substitute("Bearer {{token}} {{token}}", &fake()).expect("resolves");
    assert_eq!(resolved.text, "Bearer tok-123 tok-123");
    assert_eq!(resolved.secrets, ["tok-123"]);
}

#[test]
fn missing_names_fail_the_whole_text_each_named_once() {
    let missing =
        substitute("{{a}}/{{baseUrl}}/{{a}}/{{$processEnv HOME}}", &fake()).expect_err("fails");
    assert_eq!(missing.names, ["a", "$processEnv"]);
}

#[test]
fn what_is_no_variable_stays_as_written() {
    let text = "{ \"q\": \"{{ 1 + 1 }}\" } {{ unclosed";
    let resolved = substitute(text, &fake()).expect("resolves");
    assert_eq!(resolved.text, text);
}

#[test]
fn references_name_the_plain_variables_once() {
    assert_eq!(
        references("{{a}} {{b.c}} {{a}} {{$guid}} {{ 9x }}"),
        ["a", "b.c"]
    );
    assert!(is_name("_x-1.y"));
    assert!(!is_name("1x"));
    assert!(!is_name(""));
}
