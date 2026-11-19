//! The acceptance of RB-050-05 over the core's run, without a browser: 19 pages (the fixture
//! sitemap's 20 URLs less its nested index; `/missing` answers 404) from a local server in four
//! breakpoints through `Runs`, the render pool and the batch, with render views that load each
//! page from that server. Cancelled at half and resumed: 76 images, each recorded once, the
//! gallery written, the 404 among the problems.

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use async_trait::async_trait;
use rb_engine::chromium::batch::BatchOptions;
use rb_engine::chromium::hidden::{Loaded, RenderBackend, RenderId, RenderSpec, WaitSpec};
use rb_engine::{Emulation, EngineError};
use rb_visual::sitemap::{RunBreakpoint, RunState, ShotState};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::*;
use crate::sitemap::plan::Breakpoints;

fn fixture_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            serve(stream);
        }
    });
    origin
}

fn serve(mut stream: TcpStream) {
    let mut request = Vec::new();
    let mut chunk = [0u8; 1024];
    while !request.windows(4).any(|end| end == b"\r\n\r\n") && request.len() < 8192 {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => request.extend_from_slice(&chunk[..read]),
        }
    }
    let line = String::from_utf8_lossy(&request);
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let status = if path.contains("missing") {
        "404 Not Found"
    } else {
        "200 OK"
    };
    let answer = format!("HTTP/1.0 {status}\r\nContent-Length: 0\r\n\r\n");
    let _ = stream.write_all(answer.as_bytes());
}

fn get(url: &str) -> Option<u16> {
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let mut stream = TcpStream::connect(authority).ok()?;
    let request = format!("GET {path} HTTP/1.0\r\nHost: {authority}\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    let mut answer = String::new();
    stream.read_to_string(&mut answer).ok()?;
    answer.split_whitespace().nth(1)?.parse().ok()
}

/// Render views that load from the fixture server; a shot is a small PNG. A load waits while
/// `open` is `false`.
struct LocalViews {
    next: AtomicU32,
    loaded: Mutex<Vec<String>>,
    open: tokio::sync::watch::Sender<bool>,
}

impl Default for LocalViews {
    fn default() -> Self {
        Self {
            next: AtomicU32::new(0),
            loaded: Mutex::new(Vec::new()),
            open: tokio::sync::watch::Sender::new(true),
        }
    }
}

#[async_trait]
impl RenderBackend for LocalViews {
    async fn open(&self, _spec: &RenderSpec) -> Result<RenderId, EngineError> {
        Ok(RenderId(self.next.fetch_add(1, Ordering::Relaxed)))
    }

    async fn navigate(&self, _view: RenderId, url: &str) -> Result<Loaded, EngineError> {
        let mut open = self.open.subscribe();
        open.wait_for(|open| *open)
            .await
            .map_err(|_| EngineError::Closed)?;
        lock(&self.loaded).push(url.to_owned());
        let url = url.to_owned();
        let status = tokio::task::spawn_blocking(move || get(&url))
            .await
            .map_err(|error| EngineError::Invalid(error.to_string()))?
            .ok_or(EngineError::Closed)?;
        Ok(Loaded {
            status: Some(status),
            final_url: None,
        })
    }

    async fn wait(&self, _view: RenderId, _wait: &WaitSpec) -> Result<(), EngineError> {
        tokio::time::sleep(Duration::from_millis(2)).await;
        Ok(())
    }

    async fn screenshot(&self, _view: RenderId, _full: bool) -> Result<Vec<u8>, EngineError> {
        let image = rb_visual::Rgba::new(2, 2, [200, 200, 200, 255])
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        rb_visual::encode_png(&image).map_err(|error| EngineError::Invalid(error.to_string()))
    }

    async fn close(&self, _view: RenderId) {}
}

/// The plugin: the pages given, a gallery that lists the errors.
struct Tools {
    pages: Vec<String>,
}

impl PluginTools for Tools {
    fn discover(&self, args: &Value) -> Result<Value, String> {
        assert!(
            args.get("source").is_some(),
            "the core hands the source over"
        );
        let pages: Vec<Value> = self.pages.iter().map(|url| json!({ "url": url })).collect();
        Ok(json!({ "pages": pages }))
    }

    fn gallery(&self, run: &Value) -> Result<String, String> {
        let errors: Vec<String> = run["pages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|page| page["status"].as_u64().is_some_and(|status| status >= 400))
            .map(|page| {
                format!(
                    "HTTP {} {}",
                    page["status"],
                    page["url"].as_str().unwrap_or("")
                )
            })
            .collect();
        Ok(format!(
            "<section id=\"errors\">{}</section>",
            errors.join(";")
        ))
    }
}

/// Follows the runs; cancels the one in progress once `stop_at` shots are done.
struct Follow {
    runs: Mutex<Weak<Runs>>,
    root: std::path::PathBuf,
    stop_at: AtomicUsize,
    ended: mpsc::UnboundedSender<Ended>,
}

impl RunEvents for Follow {
    fn progress(&self, _run_id: &str, progress: Progress) {
        if progress.done >= self.stop_at.load(Ordering::SeqCst)
            && let Some(runs) = lock(&self.runs).upgrade()
        {
            let _ = runs.cancel(&self.root, None);
        }
    }

    fn ended(&self, ended: &Ended) {
        let _ = self.ended.send(ended.clone());
    }
}

fn breakpoints() -> Breakpoints {
    let mut plan = Breakpoints::default();
    for (name, width) in [
        ("mobile", 375),
        ("tablet", 768),
        ("desktop", 1280),
        ("wide", 1920),
    ] {
        plan.run.push(RunBreakpoint {
            name: name.to_owned(),
            label: name.to_owned(),
            width,
            height: 800,
            dpr: 1.0,
        });
        plan.specs.insert(
            name.to_owned(),
            RenderSpec {
                breakpoint: name.to_owned(),
                emulation: Emulation {
                    width,
                    height: 800,
                    ..Emulation::default()
                },
                session: "default".to_owned(),
            },
        );
    }
    plan
}

struct Harness {
    runs: Arc<Runs>,
    follow: Arc<Follow>,
    ended: mpsc::UnboundedReceiver<Ended>,
    views: Arc<LocalViews>,
    tools: Arc<Tools>,
    root: tempfile::TempDir,
}

fn harness(pages: Vec<String>) -> Harness {
    let root = tempfile::tempdir().expect("a project");
    let runs = Runs::new();
    let (sender, ended) = mpsc::unbounded_channel();
    let follow = Arc::new(Follow {
        runs: Mutex::new(Arc::downgrade(&runs)),
        root: root.path().to_path_buf(),
        stop_at: AtomicUsize::new(usize::MAX),
        ended: sender,
    });
    Harness {
        runs,
        follow,
        ended,
        views: Arc::new(LocalViews::default()),
        tools: Arc::new(Tools { pages }),
        root,
    }
}

impl Harness {
    fn setup(&self) -> Setup {
        Setup {
            root: self.root.path().to_path_buf(),
            now_unix: 1_778_457_600,
            batch: BatchOptions {
                parallel: 4,
                host_interval: Duration::ZERO,
                ..BatchOptions::default()
            },
            full_page: true,
            compare: None,
        }
    }

    fn services(&self) -> Services {
        Services {
            tools: self.tools.clone(),
            backend: self.views.clone(),
            events: self.follow.clone(),
        }
    }

    async fn ended(&mut self) -> Ended {
        tokio::time::timeout(Duration::from_secs(30), self.ended.recv())
            .await
            .expect("the run ends")
            .expect("an end")
    }
}

fn new_run() -> Start {
    Start::New(NewRun {
        discover: json!({ "source": { "site": "https://www.example.test" } }),
        source: "https://www.example.test/".to_owned(),
        env: Some("local".to_owned()),
        breakpoints: breakpoints(),
    })
}

fn count_pngs(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .expect("a folder")
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                count_pngs(&path)
            } else {
                usize::from(path.extension().is_some_and(|ext| ext == "png"))
            }
        })
        .sum()
}

fn fixture_pages(origin: &str) -> Vec<String> {
    let mut pages: Vec<String> = (1..=18).map(|n| format!("{origin}/page-{n}")).collect();
    pages.push(format!("{origin}/missing"));
    pages
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nineteen_pages_in_four_breakpoints_cancelled_at_half_and_resumed_give_76_images() {
    let origin = fixture_server();
    let mut harness = harness(fixture_pages(&origin));
    harness.follow.stop_at.store(38, Ordering::SeqCst);
    let started = harness
        .runs
        .start(harness.setup(), new_run(), harness.services())
        .await
        .expect("starts");
    assert_eq!(started.pending, 76);
    // The start's notice names them (RB-090-83).
    assert_eq!((started.pages, started.breakpoints), (19, 4));
    let half = harness.ended().await;
    assert_eq!(half.state, RunState::Cancelled);
    assert!(
        (38..76).contains(&half.progress.done),
        "{:?}",
        half.progress
    );
    assert!(
        half.gallery.is_some(),
        "a cancelled run has its gallery too"
    );
    let root = harness.root.path().to_path_buf();
    let saved = harness
        .runs
        .status(&root, Some(&started.run_id))
        .expect("status");
    assert_eq!(saved.state, RunState::Cancelled);
    assert_eq!(saved.progress.done, half.progress.done);

    harness.follow.stop_at.store(usize::MAX, Ordering::SeqCst);
    let specs: BTreeMap<String, RenderSpec> = breakpoints().specs;
    let resumed = harness
        .runs
        .start(
            harness.setup(),
            Start::Resume {
                run_id: started.run_id.clone(),
                specs,
            },
            harness.services(),
        )
        .await
        .expect("resumes");
    assert_eq!(resumed.run_id, started.run_id);
    assert_eq!(resumed.pending, 76 - half.progress.done);
    let finished = harness.ended().await;
    assert_eq!(finished.state, RunState::Finished);
    assert_eq!(
        (
            finished.progress.total,
            finished.progress.done,
            finished.progress.failed
        ),
        (76, 76, 0)
    );
    assert_eq!(count_pngs(&finished.folder), 76);
    let store = RunStore::open(&root, &started.run_id, false)
        .expect("opens")
        .expect("there");
    let run = store.load().expect("loads").expect("run.json");
    assert_eq!(run.state, RunState::Finished);
    for page in &run.pages {
        for (name, shot) in &page.shots {
            assert_eq!(shot.state, ShotState::Done, "{} {name}", page.url);
            assert_eq!(shot.attempts, 1, "{} {name} was taken twice", page.url);
        }
    }
    let status = harness.runs.status(&root, None).expect("the latest");
    assert_eq!(status.run_id, started.run_id);
    assert_eq!(
        harness
            .runs
            .run_id_or_latest(&root, None)
            .expect("the latest"),
        started.run_id
    );
    assert_eq!(
        harness
            .runs
            .run_id_or_latest(&root, Some("named"))
            .expect("named"),
        "named"
    );
    let missing = format!("{origin}/missing");
    assert!(
        status
            .problems
            .iter()
            .any(|problem| problem.url == missing && problem.status == Some(404))
    );
    let gallery = std::fs::read_to_string(finished.gallery.expect("a gallery")).expect("html");
    assert!(gallery.contains("HTTP 404"), "{gallery}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_run_at_a_time_and_other_schemes_never_load() {
    let origin = fixture_server();
    let pages = vec![format!("{origin}/a"), "javascript:alert(1)".to_owned()];
    let mut harness = harness(pages);
    harness.views.open.send_replace(false);
    let started = harness
        .runs
        .start(harness.setup(), new_run(), harness.services())
        .await
        .expect("starts");
    assert_eq!(
        started.pending, 4,
        "the javascript: page is refused, not queued"
    );
    let second = harness
        .runs
        .start(harness.setup(), new_run(), harness.services())
        .await;
    assert!(matches!(second, Err(RunError::Busy)));
    harness.views.open.send_replace(true);
    let ended = harness.ended().await;
    assert_eq!(ended.state, RunState::Finished);
    assert_eq!(ended.progress.failed, 4);
    assert!(
        lock(&harness.views.loaded)
            .iter()
            .all(|url| url.starts_with("http://"))
    );
    assert_eq!(harness.runs.active_id(), None);
}

#[tokio::test]
async fn no_page_and_no_run_are_errors() {
    let harness = harness(Vec::new());
    let empty = harness
        .runs
        .start(harness.setup(), new_run(), harness.services())
        .await;
    assert!(matches!(empty, Err(RunError::NoPages)));
    assert_eq!(harness.runs.active_id(), None, "the slot is free again");
    let root = harness.root.path();
    assert!(matches!(
        harness.runs.cancel(root, None),
        Err(RunError::NotFound(_))
    ));
    let missing = harness.runs.start(
        harness.setup(),
        Start::Resume {
            run_id: "2026-01-01_00-00-00".to_owned(),
            specs: BTreeMap::new(),
        },
        harness.services(),
    );
    assert!(matches!(missing.await, Err(RunError::NotFound(_))));
}
