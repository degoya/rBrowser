use super::*;

fn ok(ms: f64) -> Hit {
    Hit {
        step: 0,
        ms,
        status: Some(200),
        failed: None,
    }
}

fn failed(kind: Failed, status: Option<u16>) -> Hit {
    Hit {
        step: 1,
        ms: 5.0,
        status,
        failed: Some(kind),
    }
}

#[test]
fn a_status_tells_its_failure() {
    assert_eq!(Failed::of_status(200), None);
    assert_eq!(Failed::of_status(302), None);
    assert_eq!(Failed::of_status(404), Some(Failed::Http4xx));
    assert_eq!(Failed::of_status(503), Some(Failed::Http5xx));
    assert_eq!(Failed::Timeout.id(), "timeout");
}

#[test]
fn each_second_becomes_a_point_of_the_series() {
    let mut recorder = Recorder::new(2, 10);
    for ms in [10.0, 20.0, 30.0, 40.0] {
        recorder.record(&ok(ms));
    }
    recorder.record(&failed(Failed::Http5xx, Some(500)));
    let point = recorder.tick(1, 3);
    assert_eq!(point.t, 1);
    assert_eq!(point.requests, 5);
    assert_eq!(point.errors, 1);
    assert_eq!(point.users, 3);
    assert_eq!(point.p50, Some(20.0));
    assert_eq!(point.p99, Some(40.0));
    let quiet = recorder.tick(2, 3);
    assert_eq!((quiet.requests, quiet.p50), (0, None));
    assert_eq!(recorder.series().len(), 2);
}

#[test]
fn the_totals_count_errors_by_kind_statuses_and_steps() {
    let mut recorder = Recorder::new(2, 10);
    recorder.record(&ok(10.0));
    recorder.record(&ok(30.0));
    recorder.record(&failed(Failed::Timeout, None));
    recorder.record(&failed(Failed::Http4xx, Some(404)));
    recorder.tick(1, 1);
    let totals = recorder.totals(2.0);
    assert_eq!(totals.requests, 4);
    assert_eq!(totals.errors, 2);
    assert!((totals.error_rate - 0.5).abs() < 1e-9);
    assert!((totals.rps - 2.0).abs() < 1e-9);
    assert_eq!(recorder.errors().get("timeout"), Some(&1));
    assert_eq!(recorder.errors().get("http4xx"), Some(&1));
    assert_eq!(recorder.statuses().get(&200), Some(&2));
    assert_eq!(recorder.statuses().get(&404), Some(&1));
    let steps = recorder.steps();
    assert_eq!((steps[0].requests, steps[0].errors), (2, 0));
    assert_eq!((steps[1].requests, steps[1].errors), (2, 2));
    assert!((steps[0].mean_ms - 20.0).abs() < 1e-9);
}

#[test]
fn the_window_holds_the_last_seconds_only() {
    let mut recorder = Recorder::new(1, 2);
    recorder.record(&failed(Failed::Http5xx, Some(500)));
    recorder.tick(1, 1);
    recorder.record(&ok(100.0));
    recorder.tick(2, 1);
    recorder.record(&ok(300.0));
    recorder.tick(3, 1);
    let window = recorder.window();
    assert_eq!((window.requests, window.errors), (2, 0));
    assert_eq!(window.p95, Some(300.0));
}

#[test]
fn a_failed_request_without_an_answer_adds_no_latency() {
    let mut recorder = Recorder::new(1, 10);
    recorder.record(&Hit {
        step: 0,
        ms: 9999.0,
        status: None,
        failed: Some(Failed::Connection),
    });
    let point = recorder.tick(1, 1);
    assert_eq!((point.requests, point.errors, point.p50), (1, 1, None));
    assert_eq!(recorder.totals(1.0).latency, None);
}
