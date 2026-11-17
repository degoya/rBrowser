use super::*;
use crate::fake_host::FakeHost;

#[test]
fn each_view_reloads_then_waits_for_its_new_document() {
    let mut host = FakeHost::default();
    host.answer("1000.5"); // view 1: the old time origin
    host.answer("2000.25"); // view 2
    host.answer(r#"{"stale":true}"#); // view 1: the old document still there
    host.fail("plugins.error.host_failed"); // view 1: the context went away
    host.answer(r#"{"settled":true}"#); // view 1: settled
    host.answer(r#"{"settled":false}"#); // view 2: never went idle
    let outcomes = reload(&mut host, &[1, 2]);
    assert_eq!(
        outcomes,
        [
            Outcome {
                view: 1,
                settled: true,
                error: None
            },
            Outcome {
                view: 2,
                settled: false,
                error: None
            }
        ]
    );
    let scripts: Vec<&str> = host.evaluated.iter().map(|(_, s)| s.as_str()).collect();
    assert!(scripts[0].contains("location.reload()"));
    assert!(scripts[2].ends_with("(1000.5)"));
    assert!(scripts[5].ends_with("(2000.25)"));
    assert_eq!(host.evaluated.len(), 6);
}

#[test]
fn a_view_that_cannot_be_reloaded_or_never_answers_says_why() {
    let mut host = FakeHost::default();
    host.fail("plugins.error.not_available"); // view 3: a Firefox view
    let outcomes = reload(&mut host, &[3]);
    assert_eq!(
        outcomes[0].error.as_deref(),
        Some("plugins.error.not_available")
    );
    let mut host = FakeHost {
        tick: 10,
        ..FakeHost::default()
    };
    host.answer("1.0");
    for _ in 0..10 {
        host.fail("plugins.error.host_failed");
    }
    let outcomes = reload(&mut host, &[1]);
    assert!(!outcomes[0].settled);
    assert_eq!(
        outcomes[0].error.as_deref(),
        Some("plugins.error.host_failed")
    );
    assert!(host.evaluated.len() < 10, "the clock ends the wait");
}

#[test]
fn the_wait_script_without_an_origin_does_not_check_for_staleness() {
    let script = wait_script(None);
    assert!(script.ends_with("(null)"));
    assert!(script.contains("document.readyState === 'complete'"));
}
