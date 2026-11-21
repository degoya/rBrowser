use rb_api_model::{ApiRequest, Body, Entry};

use super::*;

const FILE: &str = r#"{
  "name": "Checkout",
  "order": "weighted",
  "steps": [
    { "request": "shop/list.http", "weight": 3, "thinkMs": { "min": 500, "max": 1500 } },
    { "request": "shop/buy.http" }
  ],
  "variables": { "user": ["ada", "bob"] },
  "profile": { "users": 20, "rampUpS": 10, "holdS": 40, "rampDownS": 10, "rps": 50 },
  "abort": { "errorRate": 0.2, "p95Ms": 2000 }
}"#;

#[test]
fn a_scenario_file_reads_with_its_defaults() {
    let scenario = Scenario::from_json(FILE).expect("reads");
    assert_eq!(scenario.name, "Checkout");
    assert_eq!(scenario.order, Order::Weighted);
    assert_eq!(scenario.steps.len(), 2);
    assert_eq!(scenario.steps[0].weight, 3);
    assert_eq!(
        scenario.steps[0].think_ms,
        Think {
            min: 500,
            max: 1500
        }
    );
    assert_eq!(scenario.steps[1].weight, 1);
    assert_eq!(scenario.steps[1].think_ms, Think::default());
    assert_eq!(scenario.profile.users, 20);
    assert_eq!(scenario.profile.rps, Some(50.0));
    assert_eq!(scenario.abort.error_rate, Some(0.2));
    assert_eq!(scenario.abort.window_s, DEFAULT_WINDOW_S);
    assert_eq!(scenario.environment, None);
}

#[test]
fn an_unknown_field_or_a_broken_file_is_refused_with_its_reason() {
    let unknown = Scenario::from_json(
        r#"{"name":"x","steps":[],"profile":{"users":1,"holdS":1},"proxy":"x"}"#,
    );
    assert!(unknown.expect_err("refused").contains("proxy"));
    assert!(Scenario::from_json("{").is_err());
}

#[test]
fn a_scenario_holds_only_with_steps_weights_and_a_sane_profile() {
    let mut scenario = Scenario::from_json(FILE).expect("reads");
    assert_eq!(scenario.check(), Ok(()));
    scenario.steps[0].weight = 0;
    assert_eq!(scenario.check(), Err("weight"));
    scenario.steps[0].weight = 1;
    scenario.steps[0].think_ms = Think { min: 9, max: 1 };
    assert_eq!(scenario.check(), Err("thinkMs"));
    scenario.steps.clear();
    assert_eq!(scenario.check(), Err("steps"));
    let mut scenario = Scenario::from_json(FILE).expect("reads");
    scenario.profile.users = 0;
    assert_eq!(scenario.check(), Err("users"));
    scenario.profile.users = 1;
    scenario.profile.hold_s = 0;
    scenario.profile.ramp_up_s = 0;
    scenario.profile.ramp_down_s = 0;
    assert_eq!(scenario.check(), Err("duration"));
    scenario.profile.hold_s = 1;
    scenario.profile.rps = Some(0.0);
    assert_eq!(scenario.check(), Err("rps"));
    scenario.profile.rps = Some(f64::NAN);
    assert_eq!(scenario.check(), Err("rps"));
    scenario.profile.rps = None;
    scenario.abort.error_rate = Some(1.5);
    assert_eq!(scenario.check(), Err("errorRate"));
    scenario.abort.error_rate = None;
    scenario.variables.insert("user".into(), Vec::new());
    assert_eq!(scenario.check(), Err("variables"));
    scenario.variables.clear();
    scenario
        .variables
        .insert("not a name".into(), vec!["x".into()]);
    assert_eq!(scenario.check(), Err("variables"));
}

#[test]
fn each_user_takes_its_own_value_in_turn() {
    let scenario = Scenario::from_json(FILE).expect("reads");
    assert_eq!(
        scenario.values_of(0),
        vec![("user".to_owned(), "ada".to_owned())]
    );
    assert_eq!(
        scenario.values_of(1),
        vec![("user".to_owned(), "bob".to_owned())]
    );
    assert_eq!(
        scenario.values_of(2),
        vec![("user".to_owned(), "ada".to_owned())]
    );
}

#[test]
fn a_users_values_go_into_every_text_of_the_request() {
    let mut request = ApiRequest::new("POST", "{{baseUrl}}/users/{{user}}?n={{$iteration}}");
    request.headers.push(Entry {
        name: "X-User".into(),
        value: "{{ user }}-{{$vu}}".into(),
        enabled: true,
    });
    request.body = Body::Json {
        text: "{\"name\": \"{{user}}\", \"token\": \"{{token}}\"}".into(),
    };
    let values = vec![("user".to_owned(), "ada".to_owned())];
    let personal = personalize(&request, &values, 7, 3);
    assert_eq!(personal.url, "{{baseUrl}}/users/ada?n=3");
    assert_eq!(personal.headers[0].value, "ada-7");
    assert_eq!(
        personal.body,
        Body::Json {
            text: "{\"name\": \"ada\", \"token\": \"{{token}}\"}".into()
        }
    );
}

#[test]
fn a_scenario_writes_back_as_it_reads() {
    let scenario = Scenario::from_json(FILE).expect("reads");
    let again = Scenario::from_json(&scenario.to_json()).expect("again");
    assert_eq!(again, scenario);
}
