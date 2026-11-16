use rb_config::{Environment, ProjectConfig};

use super::*;

fn environment(id: &str, url: &str) -> Environment {
    Environment {
        id: id.to_owned(),
        url: url.to_owned(),
        color: None,
        reload_interval_sec: None,
    }
}

fn three() -> Vec<Environment> {
    vec![
        environment("local", "http://localhost:3000"),
        environment("staging", "https://staging.example.test/app/"),
        environment("prod", "https://www.example.test"),
    ]
}

fn ids(list: &[Environment]) -> Vec<&str> {
    list.iter()
        .map(|environment| environment.id.as_str())
        .collect()
}

#[test]
fn the_environments_are_the_calls_else_the_settings_else_every_one() {
    let config = three();
    let call = vec!["prod".to_owned(), "local".to_owned(), "prod".to_owned()];
    let preset = vec!["local".to_owned(), "staging".to_owned()];
    assert_eq!(
        ids(&environments(&config, &call, &preset).expect("plans")),
        ["prod", "local"]
    );
    assert_eq!(
        ids(&environments(&config, &[], &preset).expect("plans")),
        ["local", "staging"]
    );
    assert_eq!(
        ids(&environments(&config, &[], &[]).expect("plans")),
        ["local", "staging", "prod"]
    );
}

#[test]
fn too_few_unknown_or_non_web_environments_are_refused() {
    let config = three();
    assert_eq!(
        environments(&config, &["local".to_owned()], &[]),
        Err(PlanError::TooFewEnvironments)
    );
    assert_eq!(
        environments(&config, &["local".to_owned(), "qa".to_owned()], &[]),
        Err(PlanError::UnknownEnvironment("qa".to_owned()))
    );
    let mut odd = three();
    odd[1].url = "file:///c:/site".to_owned();
    assert_eq!(
        environments(&odd, &[], &[]),
        Err(PlanError::EnvironmentUrl("staging".to_owned()))
    );
}

/// The mapping between environments as a table: what is given, the path it stands for (or the
/// error code), and the URL under `staging`.
#[test]
fn paths_and_urls_map_between_the_environments() {
    let config = three();
    let staging = &config[1].url;
    let rows: [(&str, Result<&str, &str>); 11] = [
        ("/about", Ok("/about")),
        ("/shop/cart?page=2#top", Ok("/shop/cart?page=2#top")),
        ("http://localhost:3000/about?x=1", Ok("/about?x=1")),
        ("HTTP://LOCALHOST:3000", Ok("/")),
        ("https://staging.example.test/app/blog#h", Ok("/blog#h")),
        ("https://www.example.test:443/?q", Ok("/?q")),
        // Another host is never rewritten into an environment.
        (
            "https://cdn.other.test/about",
            Err("env_compare.error.foreign_host"),
        ),
        // Under the staging host but outside its base path.
        (
            "https://staging.example.test/apps",
            Err("env_compare.error.foreign_host"),
        ),
        ("//cdn.other.test/x", Err("env_compare.error.bad_path")),
        ("about:blank", Err("env_compare.error.bad_path")),
        ("/a b", Err("env_compare.error.bad_path")),
    ];
    for (input, expected) in rows {
        let got = path_of(input, &config);
        match expected {
            Ok(path) => {
                assert_eq!(got.as_deref(), Ok(path), "{input}");
                assert_eq!(
                    url_in(staging, path),
                    format!("https://staging.example.test/app{path}"),
                    "{input}"
                );
            }
            Err(code) => assert_eq!(got.map_err(|error| error.code()), Err(code), "{input}"),
        }
    }
}

#[test]
fn a_view_stands_on_its_path_in_or_outside_the_environments() {
    let config = three();
    assert_eq!(view_path("http://localhost:3000/a?b", &config), "/a?b");
    assert_eq!(view_path("https://other.test/x/y", &config), "/x/y");
    assert_eq!(view_path("about:blank", &config), "/");
}

#[test]
fn every_two_environments_are_a_pair() {
    assert_eq!(pairs(2), [(0, 1)]);
    assert_eq!(pairs(3), [(0, 1), (0, 2), (1, 2)]);
    assert!(pairs(1).is_empty());
}

#[test]
fn the_breakpoints_are_the_calls_else_the_views_else_the_settings() {
    let list = |items: &[&str]| -> Vec<String> { items.iter().map(|s| (*s).to_owned()).collect() };
    assert_eq!(
        breakpoint_ids(&list(&["a", "a", "b"]), &list(&["c"]), &list(&["d"])),
        ["a", "b"]
    );
    assert_eq!(breakpoint_ids(&[], &list(&["c"]), &list(&["d"])), ["c"]);
    assert_eq!(breakpoint_ids(&[], &[], &list(&["d"])), ["d"]);
    assert!(breakpoint_ids(&[], &[], &[]).is_empty());
}

#[test]
fn the_settings_are_bounded_and_the_call_goes_over_them() {
    let mut config = ProjectConfig::default();
    config.visual.threshold = 0.3;
    config.env_compare.parallel = 99;
    config.env_compare.wait_ms = 99_999;
    config.env_compare.max_diff_percent = 250.0;
    config.env_compare.full_page = true;
    let plain = effective(&config, &CompareArgs::default());
    assert_eq!(plain.batch.parallel, MAX_PARALLEL);
    assert_eq!(plain.batch.wait.delay, Duration::from_millis(30_000));
    assert_eq!(plain.batch.host_interval, Duration::ZERO);
    assert!(plain.batch.full_page);
    assert!(
        (plain.options.threshold - 0.3).abs() < f64::EPSILON,
        "visual.threshold"
    );
    assert!((plain.max_ratio - 1.0).abs() < f64::EPSILON);
    config.env_compare.threshold = Some(0.2);
    let call = CompareArgs {
        full_page: Some(false),
        threshold: Some(0.05),
        ..CompareArgs::default()
    };
    let over = effective(&config, &call);
    assert!(!over.batch.full_page);
    assert!((over.options.threshold - 0.05).abs() < f64::EPSILON);
    assert!(
        (effective(&config, &CompareArgs::default())
            .options
            .threshold
            - 0.2)
            .abs()
            < 1e-9
    );
}
