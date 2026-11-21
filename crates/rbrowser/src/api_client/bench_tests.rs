use rb_api_model::ApiRequest;

use super::*;
use crate::api_client::fakes::{UrlSender, answer, context, no_env};
use crate::api_client::store::Store;

fn saved(store: &Store, url: &str) -> String {
    let mut request = ApiRequest::new("GET", url);
    request.name = "Health".into();
    store.save(None, None, &request).expect("saved")
}

#[test]
fn two_environments_are_sent_as_often_as_asked_and_compared() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    let id = saved(&store, "{{baseUrl}}/health");
    let sender = UrlSender::new(vec![
        ("https://staging.example.com/", answer(200, &[], "ok", 40.0)),
        ("https://api.example.com/", answer(200, &[], "ok", 10.0)),
    ]);
    let ctx = context(dir.path());
    let courier = Courier {
        ctx: &ctx,
        store: &store,
        sender: &sender,
        env: &no_env,
    };
    let bench = run(
        courier,
        &id,
        vec![Some("staging".into()), Some("production".into())],
        5,
        2,
    )
    .expect("ran");
    let urls = sender.urls();
    assert_eq!(urls.len(), 10);
    assert_eq!(
        urls.iter()
            .filter(|url| url.starts_with("https://staging."))
            .count(),
        5
    );
    assert_eq!(bench.series.len(), 2);
    let staging = &bench.series[0];
    assert_eq!(staging.environment.as_deref(), Some("staging"));
    assert_eq!(staging.sent, 5);
    assert_eq!(staging.statuses.get(&200), Some(&5));
    let total = staging.phases.total.expect("totals");
    assert_eq!((total.count, total.median, total.p95), (5, 40.0, 40.0));
    assert_eq!(staging.phases.tls, None);
    assert_eq!(staging.phases.dns.map(|dns| dns.max), Some(1.0));
    let comparison = bench.comparison.expect("compared");
    assert_eq!(comparison.median_delta, -30.0);
}

#[test]
fn repeats_and_concurrency_are_held_low() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    let id = saved(&store, "https://api.example.com/");
    let sender = UrlSender::new(vec![(
        "https://api.example.com/",
        answer(200, &[], "", 1.0),
    )]);
    let ctx = context(dir.path());
    let courier = Courier {
        ctx: &ctx,
        store: &store,
        sender: &sender,
        env: &no_env,
    };
    let bench = run(courier, &id, Vec::new(), 10_000, 64).expect("ran");
    assert_eq!(bench.repeats, MAX_REPEATS);
    assert_eq!(bench.concurrency, MAX_CONCURRENCY);
    assert_eq!(sender.urls().len(), MAX_REPEATS as usize);
    let none = run(courier, &id, Vec::new(), 0, 0).expect("ran");
    assert_eq!((none.repeats, none.concurrency), (1, 1));
}

#[test]
fn a_host_not_granted_fails_before_anything_is_repeated() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    let id = saved(&store, "https://elsewhere.example.net/");
    let sender = UrlSender::new(Vec::new());
    let ctx = context(dir.path());
    let courier = Courier {
        ctx: &ctx,
        store: &store,
        sender: &sender,
        env: &no_env,
    };
    let failure = run(courier, &id, Vec::new(), 5, 1).expect_err("refused");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.host_not_allowed")
    );
    assert!(sender.urls().is_empty());
    let three = vec![None, None, None];
    let failure = run(courier, &id, three, 5, 1).expect_err("refused");
    assert_eq!(
        failure.code(),
        Some("plugin.api-client.errors.too_many_environments")
    );
}

#[test]
fn sends_without_an_answer_are_counted_by_their_code() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::new(dir.path());
    let id = saved(&store, "https://api.example.com/");
    let sender = UrlSender::new(Vec::new());
    let ctx = context(dir.path());
    let courier = Courier {
        ctx: &ctx,
        store: &store,
        sender: &sender,
        env: &no_env,
    };
    let bench = run(courier, &id, Vec::new(), 3, 1).expect("ran");
    let series = &bench.series[0];
    assert_eq!(
        series.errors.get("plugin.api-client.errors.send_failed"),
        Some(&3)
    );
    assert_eq!(series.phases.total, None);
    assert_eq!(bench.comparison, None);
}
