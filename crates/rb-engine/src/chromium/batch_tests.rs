use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

use super::*;
use crate::Emulation;
use crate::chromium::fake::FakeBackend;

const BREAKPOINTS: [&str; 4] = ["mobile", "tablet", "desktop", "wide"];

#[derive(Default)]
struct Recorder {
    results: Mutex<Vec<(BatchTask, ShotResult)>>,
}

#[async_trait]
impl BatchSink for Recorder {
    async fn finished(&self, task: &BatchTask, result: ShotResult) {
        lock(&self.results).push((task.clone(), result));
    }
}

fn spec(breakpoint: &str) -> RenderSpec {
    RenderSpec {
        breakpoint: breakpoint.to_owned(),
        emulation: Emulation::default(),
        session: "default".to_owned(),
    }
}

/// `pages` pages times the four breakpoints, breakpoint by breakpoint.
fn tasks(pages: &[&str]) -> Vec<BatchTask> {
    let mut all = Vec::new();
    for breakpoint in BREAKPOINTS {
        for url in pages {
            all.push(BatchTask {
                key: all.len(),
                url: (*url).to_owned(),
                spec: spec(breakpoint),
            });
        }
    }
    all
}

fn options(parallel: usize) -> BatchOptions {
    BatchOptions {
        parallel,
        host_interval: Duration::ZERO,
        retries: 0,
        ..BatchOptions::default()
    }
}

async fn run(
    backend: &Arc<FakeBackend>,
    tasks: Vec<BatchTask>,
    options: BatchOptions,
    cancel: Arc<Cancel>,
) -> (BatchSummary, Vec<(BatchTask, ShotResult)>) {
    let pool = RenderPool::new(backend.clone(), options.parallel);
    let sink = Arc::new(Recorder::default());
    let summary = run_batch(pool.clone(), tasks, options, sink.clone(), cancel).await;
    pool.shutdown().await;
    let results = std::mem::take(&mut *lock(&sink.results));
    (summary, results)
}

#[tokio::test]
async fn every_page_in_every_breakpoint_is_taken_once() {
    let backend = Arc::new(FakeBackend::default());
    let pages = [
        "https://a.test/",
        "https://a.test/missing",
        "https://a.test/b",
    ];
    let (summary, results) = run(&backend, tasks(&pages), options(4), Arc::default()).await;
    assert_eq!(summary.done, 12);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.left, 0);
    assert!(!summary.cancelled);
    let keys: BTreeSet<usize> = results.iter().map(|(task, _)| task.key).collect();
    assert_eq!(keys.len(), 12);
    for (task, result) in &results {
        let png = result.png.as_ref().expect("a shot");
        let text = String::from_utf8(png.clone()).expect("text");
        assert_eq!(text, format!("{}@{}@true", task.url, task.spec.breakpoint));
        let status = result.loaded.as_ref().and_then(|loaded| loaded.status);
        let wanted = if task.url.contains("missing") {
            404
        } else {
            200
        };
        assert_eq!(status, Some(wanted));
    }
    assert!(lock(&backend.views).is_empty(), "every view is closed");
    assert!(backend.opened.load(Ordering::Relaxed) <= 12);
}

#[tokio::test]
async fn no_more_pages_load_at_once_than_asked() {
    let backend = Arc::new(FakeBackend::with_delay(Duration::from_millis(20)));
    let pages = ["https://a.test/1", "https://b.test/2", "https://c.test/3"];
    let (summary, _) = run(&backend, tasks(&pages), options(2), Arc::default()).await;
    assert_eq!(summary.done, 12);
    assert_eq!(backend.most_at_once.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_failure_is_retried_and_reported_after_the_last_attempt() {
    let backend = Arc::new(FakeBackend::default());
    backend.fail("https://a.test/flaky", 1);
    backend.fail("https://a.test/broken", 10);
    let mut options = options(1);
    options.retries = 2;
    let all: Vec<BatchTask> = tasks(&["https://a.test/flaky", "https://a.test/broken"])
        .into_iter()
        .filter(|task| task.spec.breakpoint == "mobile")
        .collect();
    let (summary, results) = run(&backend, all, options, Arc::default()).await;
    assert_eq!((summary.done, summary.failed), (1, 1));
    let flaky = &results[0].1;
    assert!(flaky.png.is_ok());
    assert_eq!(flaky.attempts, 2);
    let broken = &results[1].1;
    assert_eq!(broken.attempts, 3);
    let error = broken.png.as_ref().expect_err("failed");
    assert!(error.starts_with("navigate: "), "{error}");
}

#[tokio::test]
async fn a_page_that_takes_too_long_times_out() {
    let backend = Arc::new(FakeBackend::with_delay(Duration::from_millis(200)));
    let mut options = options(1);
    options.wait.timeout = Duration::from_millis(20);
    let one = vec![tasks(&["https://a.test/slow"]).remove(0)];
    let (summary, results) = run(&backend, one, options, Arc::default()).await;
    assert_eq!(summary.failed, 1);
    assert_eq!(results[0].1.png, Err("timeout".to_owned()));
}

#[tokio::test]
async fn loads_of_one_host_are_spaced_by_the_interval() {
    let backend = Arc::new(FakeBackend::default());
    let mut options = options(4);
    options.host_interval = Duration::from_millis(40);
    let one_host: Vec<BatchTask> = tasks(&["https://a.test/1", "https://a.test/2"])
        .into_iter()
        .take(4)
        .collect();
    let (summary, _) = run(&backend, one_host, options, Arc::default()).await;
    assert_eq!(summary.done, 4);
    let visits = lock(&backend.visits).clone();
    let mut times: Vec<Instant> = visits.iter().map(|(_, when)| *when).collect();
    times.sort();
    for pair in times.windows(2) {
        assert!(
            pair[1] - pair[0] >= Duration::from_millis(35),
            "{:?}",
            pair[1] - pair[0]
        );
    }
}

#[tokio::test]
async fn a_cancelled_batch_reports_nothing_twice_and_resumes_with_the_rest() {
    let backend = Arc::new(FakeBackend::with_delay(Duration::from_millis(30)));
    let pages = [
        "https://a.test/1",
        "https://a.test/2",
        "https://a.test/3",
        "https://a.test/4",
        "https://a.test/5",
    ];
    let all = tasks(&pages);
    let cancel = Arc::new(Cancel::default());
    let stopper = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        stopper.cancel();
    });
    let (first, taken) = run(&backend, all.clone(), options(2), cancel).await;
    assert!(first.cancelled);
    assert!(first.done > 0 && first.done < 20, "{first:?}");
    assert_eq!(first.done + first.failed + first.left, 20);
    let done: BTreeSet<usize> = taken.iter().map(|(task, _)| task.key).collect();
    assert_eq!(done.len(), taken.len(), "nothing reported twice");
    let rest: Vec<BatchTask> = all
        .iter()
        .filter(|task| !done.contains(&task.key))
        .cloned()
        .collect();
    let (second, again) = run(&backend, rest, options(2), Arc::default()).await;
    assert_eq!(second.done, 20 - first.done);
    let mut every: Vec<usize> = done.into_iter().collect();
    every.extend(again.iter().map(|(task, _)| task.key));
    every.sort_unstable();
    assert_eq!(every, (0..20).collect::<Vec<_>>());
}

#[tokio::test]
async fn a_batch_cancelled_while_its_views_open_leaves_none_open() {
    let backend = Arc::new(FakeBackend::with_open_close(
        Duration::from_millis(100),
        Duration::ZERO,
    ));
    let cancel = Arc::new(Cancel::default());
    let stopper = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        stopper.cancel();
    });
    let (summary, taken) = run(&backend, tasks(&["https://a.test/"]), options(2), cancel).await;
    assert!(summary.cancelled && taken.is_empty());
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(
        lock(&backend.views).is_empty(),
        "the views still opening when the batch stopped are closed"
    );
}

#[tokio::test]
async fn a_view_that_never_opens_times_out() {
    let backend = Arc::new(FakeBackend::with_open_close(
        Duration::from_millis(300),
        Duration::ZERO,
    ));
    let mut options = options(1);
    options.wait.timeout = Duration::from_millis(20);
    let one = vec![tasks(&["https://a.test/"]).remove(0)];
    let (summary, results) = run(&backend, one, options, Arc::default()).await;
    assert_eq!(summary.failed, 1);
    // The timeout, not the open that would have succeeded 300 ms later: no wait for the open.
    assert_eq!(results[0].1.png, Err("open: timeout".to_owned()));
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(lock(&backend.views).is_empty(), "the late view is closed");
}

#[test]
fn host_of_takes_the_authority_without_credentials() {
    assert_eq!(
        host_of("https://WWW.Example.test:8080/a?b"),
        "www.example.test:8080"
    );
    assert_eq!(host_of("http://user:pw@a.test#x"), "a.test");
    assert_eq!(host_of("about:blank"), "");
}
