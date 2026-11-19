use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rb_engine::Emulation;
use rb_engine::chromium::hidden::{Loaded, RenderSpec};
use rb_visual::sitemap::{RunBreakpoint, ShotState};
use rb_visual::{Rgba, encode_png};

use super::*;

fn png(color: [u8; 4]) -> Vec<u8> {
    encode_png(&Rgba::new(4, 4, color).expect("image")).expect("png")
}

fn setup(root: &std::path::Path) -> (Arc<Mutex<SitemapRun>>, RunStore, Vec<Task>) {
    let breakpoints = vec![RunBreakpoint {
        name: "mobile".to_owned(),
        label: "Mobile".to_owned(),
        width: 4,
        height: 4,
        dpr: 1.0,
    }];
    let pages = vec![
        (
            "http://127.0.0.1:8080/".to_owned(),
            Some("https://www.example.test/".to_owned()),
        ),
        ("http://127.0.0.1:8080/missing".to_owned(), None),
    ];
    let run = SitemapRun::new("r", "https://www.example.test/", breakpoints, &pages, 0);
    let tasks = run.tasks(false, 3);
    let store = RunStore::open(root, "r", true)
        .expect("opens")
        .expect("made");
    (Arc::new(Mutex::new(run)), store, tasks)
}

fn batch_task(key: usize) -> BatchTask {
    BatchTask {
        key,
        url: String::new(),
        spec: RenderSpec {
            breakpoint: "mobile".to_owned(),
            emulation: Emulation::default(),
            session: "default".to_owned(),
        },
    }
}

fn result(png: Result<Vec<u8>, String>, status: u16) -> ShotResult {
    ShotResult {
        loaded: Some(Loaded {
            status: Some(status),
            final_url: None,
        }),
        png,
        elapsed: Duration::from_millis(5),
        attempts: 1,
    }
}

#[test]
fn a_failed_shot_is_logged_without_the_pages_query() {
    let failed = FailedShot::new(
        "https://user:secret@www.example.test/shop?token=abc#top",
        "mobile",
        "screenshot: no frame within 15 s",
    );
    assert_eq!(failed.url, "https://www.example.test/shop");
    assert_eq!(failed.breakpoint, "mobile");
    assert_eq!(failed.error, "screenshot: no frame within 15 s");
}

#[tokio::test]
async fn a_failed_shot_keeps_its_reason_in_run_json() {
    let root = tempfile::tempdir().expect("temp");
    let (run, store, tasks) = setup(root.path());
    let sink = RunSink::new(run, store.clone(), tasks, None, 4, Box::new(|_| {}));
    sink.finished(
        &batch_task(0),
        result(Err("screenshot: no frame within 15 s".to_owned()), 200),
    )
    .await;
    sink.flush().await.expect("flushes");
    let saved = store.load().expect("loads").expect("run.json");
    let shot = &saved.pages[0].shots["mobile"];
    assert_eq!(shot.state, ShotState::Failed);
    assert_eq!(
        shot.error.as_deref(),
        Some("screenshot: no frame within 15 s")
    );
}

#[tokio::test]
async fn a_shot_is_written_recorded_and_reported() {
    let root = tempfile::tempdir().expect("temp");
    let (run, store, tasks) = setup(root.path());
    let reports = Arc::new(AtomicUsize::new(0));
    let counter = reports.clone();
    let sink = RunSink::new(
        run.clone(),
        store.clone(),
        tasks,
        None,
        4,
        Box::new(move |_| {
            counter.fetch_add(1, Ordering::Relaxed);
        }),
    );
    sink.finished(&batch_task(0), result(Ok(png([1, 2, 3, 255])), 200))
        .await;
    sink.finished(&batch_task(1), result(Err("timeout".to_owned()), 404))
        .await;
    sink.finished(&batch_task(9), result(Err("x".to_owned()), 200))
        .await;
    assert_eq!(reports.load(Ordering::Relaxed), 2);
    assert!(store.dir().join("index/mobile.png").is_file());
    // The first shot writes run.json, the second within SAVE_EVERY does not.
    let throttled = store.load().expect("loads").expect("run.json");
    assert_eq!(throttled.pages[0].shots["mobile"].state, ShotState::Done);
    assert_eq!(throttled.pages[1].shots["mobile"].state, ShotState::Pending);
    sink.flush().await.expect("flushes");
    let saved = store.load().expect("loads").expect("run.json");
    assert_eq!(saved, *lock(&run));
    assert_eq!(saved.pages[0].shots["mobile"].state, ShotState::Done);
    assert_eq!(saved.pages[1].status, Some(404));
    assert_eq!(
        saved.pages[1].shots["mobile"].error.as_deref(),
        Some("timeout")
    );
}

#[tokio::test]
async fn compare_names_the_baseline_by_the_sitemap_url_and_writes_the_diff() {
    let root = tempfile::tempdir().expect("temp");
    let (run, store, tasks) = setup(root.path());
    let baselines = BaselineStore::new(root.path());
    baselines
        .write(
            "index",
            "mobile",
            &png([0, 0, 0, 255]),
            &BaselineMeta::default(),
        )
        .expect("a baseline");
    let sink = RunSink::new(
        run.clone(),
        store.clone(),
        tasks,
        Some(CompareWith {
            store: baselines,
            strategy: SlugStrategy::Path,
            options: DiffOptions {
                threshold: 0.1,
                include_aa: false,
            },
            machine: BaselineMeta::default(),
            full_page: true,
        }),
        1,
        Box::new(|_| {}),
    );
    sink.finished(&batch_task(0), result(Ok(png([255, 255, 255, 255])), 200))
        .await;
    sink.finished(&batch_task(1), result(Ok(png([1, 1, 1, 255])), 404))
        .await;
    let run = lock(&run).clone();
    let differs = run.pages[0].shots["mobile"]
        .compare
        .clone()
        .expect("compared");
    assert_eq!(differs.status, "differs");
    assert!(differs.ratio > 0.9);
    assert_eq!(differs.diff_file.as_deref(), Some("index/mobile.diff.png"));
    assert!(store.dir().join("index/mobile.diff.png").is_file());
    let new = run.pages[1].shots["mobile"]
        .compare
        .clone()
        .expect("compared");
    assert_eq!(new.status, "new");
}
