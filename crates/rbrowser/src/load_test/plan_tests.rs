use rb_api_model::ApiRequest;
use serde_json::json;

use super::*;
use crate::api_client::ops::Settings;

fn no_env(_name: &str) -> Option<String> {
    None
}

fn context(root: &std::path::Path) -> Context {
    Context {
        api: ApiContext {
            root: Some(root.to_path_buf()),
            trusted: true,
            environment: Some("staging".into()),
            environments: vec![("staging".into(), "https://staging.example.com".into())],
            allowed_hosts: Vec::new(),
            secrets: json!({}),
            settings: Settings {
                history_size: 0,
                max_response_bytes: 4096,
                bench_repeats: 1,
                bench_concurrency: 1,
                code: crate::api_client::export::CodeSettings::default(),
            },
            now: 1_700_000_000,
        },
        start_url: Some("https://shop.example.com/".into()),
        limits: Limits::default(),
        confirmed: Vec::new(),
    }
}

fn project(urls: &[&str], scenario: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp");
    let store = Store::new(dir.path());
    for (n, url) in urls.iter().enumerate() {
        store
            .save(
                Some(&format!("r{n}.http")),
                None,
                &ApiRequest::new("GET", url),
            )
            .expect("request saved");
    }
    let folder = dir.path().join(".rbrowser/api/load");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("s.json"), scenario).expect("scenario");
    dir
}

const ONE: &str = r#"{"name":"S","steps":[{"request":"r0.http"}],"profile":{"users":10,"rampUpS":5,"holdS":20,"rampDownS":5}}"#;

#[test]
fn the_summary_names_targets_users_rate_and_duration() {
    let dir = project(&["{{baseUrl}}/list"], ONE);
    let ctx = context(dir.path());
    let planned = plan(&ctx, dir.path(), "s.json", &no_env).expect("planned");
    let summary = &planned.summary;
    assert_eq!(
        summary.targets,
        vec![TargetHost {
            host: "staging.example.com".into(),
            standing: Standing::Own
        }]
    );
    assert!(summary.unconfirmed.is_empty());
    assert_eq!((summary.users, summary.duration_s), (10, 30));
    assert_eq!(summary.rps, Limits::default().max_rps);
    assert_eq!(summary.environment.as_deref(), Some("staging"));
    assert_eq!(
        summary.confirm.targets,
        vec!["staging.example.com".to_owned()]
    );
    let launch = launch(
        &ctx,
        dir.path(),
        planned,
        &summary_confirm(&ctx, &dir),
        &no_env,
    )
    .expect("launched");
    assert!(
        launch
            .live
            .ctx
            .allowed_hosts
            .contains(&"staging.example.com".to_owned())
    );
    assert_eq!(launch.plan.rps, Some(Limits::default().max_rps));
    assert_eq!(launch.meta.steps, vec!["r0.http".to_owned()]);
}

fn summary_confirm(ctx: &Context, dir: &tempfile::TempDir) -> Confirm {
    plan(ctx, dir.path(), "s.json", &no_env)
        .expect("planned")
        .summary
        .confirm
}

#[test]
fn a_foreign_host_runs_only_after_the_person_confirmed_it() {
    let dir = project(&["https://other.example.net/x"], ONE);
    let mut ctx = context(dir.path());
    let planned = plan(&ctx, dir.path(), "s.json", &no_env).expect("planned");
    assert_eq!(
        planned.summary.unconfirmed,
        vec!["other.example.net".to_owned()]
    );
    let confirm = planned.summary.confirm.clone();
    let failure = launch(&ctx, dir.path(), planned, &confirm, &no_env)
        .err()
        .expect("refused");
    assert_eq!(
        failure.code(),
        Some("plugin.load-test.errors.host_not_confirmed")
    );
    assert_eq!(failure.param("host"), Some("other.example.net"));
    ctx.confirmed = vec!["other.example.net".into()];
    let planned = plan(&ctx, dir.path(), "s.json", &no_env).expect("planned");
    assert_eq!(planned.summary.targets[0].standing, Standing::Confirmed);
    let launch = launch(&ctx, dir.path(), planned, &confirm, &no_env).expect("launched");
    assert!(
        launch
            .live
            .ctx
            .allowed_hosts
            .contains(&"other.example.net".to_owned())
    );
}

#[test]
fn every_users_values_count_for_the_targets() {
    let scenario = r#"{"name":"S","steps":[{"request":"r0.http"}],"variables":{"host":["localhost","evil.example"]},"profile":{"users":2,"holdS":5}}"#;
    let dir = project(&["http://{{host}}/x"], scenario);
    let planned = plan(&context(dir.path()), dir.path(), "s.json", &no_env).expect("planned");
    assert_eq!(planned.summary.unconfirmed, vec!["evil.example".to_owned()]);
    assert_eq!(
        planned.hosts,
        vec!["evil.example".to_owned(), "localhost".to_owned()]
    );
}

#[test]
fn a_scenario_over_a_ceiling_names_the_setting() {
    let scenario =
        r#"{"name":"S","steps":[{"request":"r0.http"}],"profile":{"users":51,"holdS":5}}"#;
    let dir = project(&["http://localhost/x"], scenario);
    let failure = plan(&context(dir.path()), dir.path(), "s.json", &no_env)
        .err()
        .expect("over");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.over_limit"));
    assert_eq!(failure.param("setting"), Some("load-test.maxUsers"));
    assert_eq!(failure.param("limit"), Some("50"));
}

#[test]
fn a_changed_summary_does_not_start() {
    let dir = project(&["http://localhost/x"], ONE);
    let ctx = context(dir.path());
    let planned = plan(&ctx, dir.path(), "s.json", &no_env).expect("planned");
    let mut confirm = planned.summary.confirm.clone();
    confirm.users = 5;
    let failure = launch(&ctx, dir.path(), planned, &confirm, &no_env)
        .err()
        .expect("changed");
    assert_eq!(
        failure.code(),
        Some("plugin.load-test.errors.summary_changed")
    );
}

#[test]
fn a_variable_without_a_value_or_a_missing_request_is_said() {
    let dir = project(&["{{nowhere}}/x"], ONE);
    let failure = plan(&context(dir.path()), dir.path(), "s.json", &no_env)
        .err()
        .expect("unresolved");
    assert_eq!(failure.code(), Some("plugin.load-test.errors.unresolved"));
    assert_eq!(failure.param("names"), Some("nowhere"));
    let missing =
        r#"{"name":"S","steps":[{"request":"gone.http"}],"profile":{"users":1,"holdS":5}}"#;
    let dir = project(&[], missing);
    let failure = plan(&context(dir.path()), dir.path(), "s.json", &no_env)
        .err()
        .expect("missing");
    assert_eq!(
        failure.code(),
        Some("plugin.load-test.errors.request_unreadable")
    );
    assert_eq!(failure.param("id"), Some("gone.http"));
}
