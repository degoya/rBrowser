//! The acceptance of RB-050-05 without a browser: the fixture site's production sitemap
//! (robots.txt, an index, a nested index, a gzip sitemap - 19 pages, `/missing` a 404) is read
//! by the plugin, rewritten to a local fixture server (`--env local`), and captured in four
//! breakpoints through the core's render pool and batch with a fake render backend that loads
//! each page from that server. The run is cancelled half way and resumed: 76 images, each taken
//! once, `run.json`, and a gallery that lists the 404.

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_engine::chromium::batch::{
    BatchOptions, BatchSink, BatchTask, Cancel, ShotResult, run_batch,
};
use rb_engine::chromium::hidden::{
    Loaded, RenderBackend, RenderId, RenderPool, RenderSpec, WaitSpec,
};
use rb_engine::{Emulation, EngineError};
use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, HttpResponse};
use rb_visual::sitemap::{Outcome, RunBreakpoint, RunState, SitemapRun, Task};
use rb_visual::sitemap_store::{GALLERY_FILE, RunStore};
use serde_json::{Value, json};
use sitemap_capture::SitemapCapture;

const PROD: &str = "https://www.example.test";
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/sitemap");
const SITEMAP_FILES: [&str; 5] = [
    "robots.txt",
    "sitemap_index.xml",
    "sitemap-pages.xml",
    "sitemap-nested-index.xml",
    "sitemap-blog.xml.gz",
];

/// The production site, as the plugin's `http` capability sees it.
fn production() -> MemoryHost {
    let mut host = MemoryHost::new().grant(Capability::Http);
    for name in SITEMAP_FILES {
        let body = std::fs::read(Path::new(FIXTURES).join(name)).expect("a fixture");
        host.responses.insert(
            format!("{PROD}/{name}"),
            HttpResponse {
                status: 200,
                headers: Vec::new(),
                body,
            },
        );
    }
    host
}

/// The local fixture server: every page answers 200 with a little HTML, `/missing` 404.
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
    let (status, body) = if path.contains("missing") {
        ("404 Not Found", "<h1>Not found</h1>".to_owned())
    } else {
        ("200 OK", format!("<h1>{path}</h1>"))
    };
    let answer = format!(
        "HTTP/1.0 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(answer.as_bytes());
}

/// GETs `url` from the fixture server; its status.
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

/// Render views that load their page from the fixture server; a shot is a PNG of the
/// breakpoint's width.
#[derive(Default)]
struct LocalBackend {
    next: AtomicU32,
    views: Mutex<BTreeMap<RenderId, u32>>,
}

#[async_trait]
impl RenderBackend for LocalBackend {
    async fn open(&self, spec: &RenderSpec) -> Result<RenderId, EngineError> {
        let id = RenderId(self.next.fetch_add(1, Ordering::Relaxed));
        let mut views = self.views.lock().expect("views");
        views.insert(id, spec.emulation.width);
        Ok(id)
    }

    async fn navigate(&self, _view: RenderId, url: &str) -> Result<Loaded, EngineError> {
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
        Ok(())
    }

    async fn screenshot(&self, view: RenderId, _full_page: bool) -> Result<Vec<u8>, EngineError> {
        let width = *self
            .views
            .lock()
            .expect("views")
            .get(&view)
            .ok_or(EngineError::Closed)?;
        let image = rb_visual::Rgba::new(width / 100, 2, [200, 200, 200, 255])
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        rb_visual::encode_png(&image).map_err(|error| EngineError::Invalid(error.to_string()))
    }

    async fn close(&self, view: RenderId) {
        self.views.lock().expect("views").remove(&view);
    }
}

/// Writes each shot and `run.json` as the core does; cancels after `stop_after` shots.
struct Sink {
    run: Mutex<SitemapRun>,
    store: RunStore,
    tasks: Vec<Task>,
    written: Mutex<BTreeMap<(usize, String), u32>>,
    stop_after: usize,
    cancel: Arc<Cancel>,
}

#[async_trait]
impl BatchSink for Sink {
    async fn finished(&self, task: &BatchTask, result: ShotResult) {
        let target = &self.tasks[task.key];
        let mut run = self.run.lock().expect("run");
        let slug = run.pages[target.page].slug.clone();
        let file = result.png.and_then(|png| {
            self.store
                .write_shot(&slug, &target.breakpoint, &png)
                .map_err(|error| error.to_string())
        });
        if file.is_ok() {
            let mut written = self.written.lock().expect("written");
            *written
                .entry((target.page, target.breakpoint.clone()))
                .or_default() += 1;
        }
        run.record(
            target,
            Outcome {
                status: result.loaded.as_ref().and_then(|loaded| loaded.status),
                final_url: None,
                result: file,
                ms: 1,
            },
        );
        self.store.save(&run).expect("run.json");
        let done = run.progress(4).done;
        if done >= self.stop_after {
            self.cancel.cancel();
        }
    }
}

fn breakpoints() -> Vec<RunBreakpoint> {
    [
        ("mobile", 375),
        ("tablet", 768),
        ("desktop", 1280),
        ("wide", 1920),
    ]
    .iter()
    .map(|(name, width)| RunBreakpoint {
        name: (*name).to_owned(),
        label: (*name).to_owned(),
        width: *width,
        height: 800,
        dpr: 1.0,
    })
    .collect()
}

/// Runs the open tasks of `run` until `stop_after` shots are done; the run as it stands.
async fn capture(
    run: SitemapRun,
    store: &RunStore,
    stop_after: usize,
    written: &Mutex<BTreeMap<(usize, String), u32>>,
) -> SitemapRun {
    let tasks = run.tasks(true, 3);
    let batch: Vec<BatchTask> = tasks
        .iter()
        .enumerate()
        .map(|(key, task)| {
            let breakpoint = run
                .breakpoints
                .iter()
                .find(|breakpoint| breakpoint.name == task.breakpoint)
                .expect("a breakpoint");
            BatchTask {
                key,
                url: run.pages[task.page].url.clone(),
                spec: RenderSpec {
                    breakpoint: breakpoint.name.clone(),
                    emulation: Emulation {
                        width: breakpoint.width,
                        height: breakpoint.height,
                        ..Emulation::default()
                    },
                    session: "default".to_owned(),
                },
            }
        })
        .collect();
    let cancel = Arc::new(Cancel::default());
    let sink = Arc::new(Sink {
        run: Mutex::new(run),
        store: store.clone(),
        tasks,
        written: Mutex::new(std::mem::take(&mut *written.lock().expect("written"))),
        stop_after,
        cancel: cancel.clone(),
    });
    let pool = RenderPool::new(Arc::new(LocalBackend::default()), 4);
    let options = BatchOptions {
        parallel: 4,
        host_interval: std::time::Duration::ZERO,
        ..BatchOptions::default()
    };
    let summary = run_batch(pool.clone(), batch, options, sink.clone(), cancel).await;
    pool.shutdown().await;
    let mut run = sink.run.lock().expect("run").clone();
    run.state = if summary.cancelled {
        RunState::Cancelled
    } else {
        RunState::Finished
    };
    store.save(&run).expect("run.json");
    *written.lock().expect("written") = std::mem::take(&mut *sink.written.lock().expect("w"));
    run
}

fn count_pngs(dir: &Path) -> usize {
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn twenty_sitemap_urls_in_four_breakpoints_give_76_images_and_a_gallery() {
    let local = fixture_server();
    let mut production = production();
    let args = json!({
        "source": { "site": PROD },
        "selection": { "respectRobots": true },
        "rewrite": { "to": local }
    });
    let answer: Value = serde_json::from_str(
        &SitemapCapture
            .call_tool(&mut production, "discover", &args.to_string())
            .expect("discovers"),
    )
    .expect("JSON");
    let sitemaps = answer["sitemaps"].as_array().expect("sitemaps");
    assert_eq!(
        sitemaps.len(),
        4,
        "the index, two sitemaps and the nested index"
    );
    let nested = sitemaps
        .iter()
        .filter(|report| {
            report["url"]
                .as_str()
                .is_some_and(|url| url.ends_with("nested-index.xml"))
        })
        .count();
    assert_eq!(answer["counts"]["found"].as_u64(), Some(19));
    assert_eq!(
        19 + nested,
        20,
        "20 sitemap URLs: 19 pages and the nested index"
    );
    let pages: Vec<(String, Option<String>)> = answer["pages"]
        .as_array()
        .expect("pages")
        .iter()
        .map(|page| {
            (
                page["url"].as_str().expect("url").to_owned(),
                page["sitemapUrl"].as_str().map(str::to_owned),
            )
        })
        .collect();
    assert!(
        pages.iter().all(|(url, _)| url.starts_with(&local)),
        "rewritten to the fixture server"
    );
    assert!(
        pages
            .iter()
            .all(|(_, from)| from.as_deref().is_some_and(|from| from.starts_with(PROD)))
    );

    let root = tempfile::tempdir().expect("a project");
    let run_id = "2026-11-11_00-00-00";
    let store = RunStore::open(root.path(), run_id, true)
        .expect("opens")
        .expect("made");
    let run = SitemapRun::new(run_id, PROD, breakpoints(), &pages, 0);
    store.save(&run).expect("run.json");
    let written = Mutex::new(BTreeMap::new());

    let half = capture(run, &store, 38, &written).await;
    assert_eq!(half.state, RunState::Cancelled);
    let after_half = half.progress(4).done;
    assert!((38..76).contains(&after_half), "{after_half}");

    let resumed = store.load().expect("loads").expect("run.json");
    assert_eq!(resumed.progress(4).done, after_half);
    let finished = capture(resumed, &store, usize::MAX, &written).await;
    assert_eq!(finished.state, RunState::Finished);
    let progress = finished.progress(4);
    assert_eq!(
        (progress.total, progress.done, progress.failed),
        (76, 76, 0)
    );
    let written = written.lock().expect("written");
    assert_eq!(written.len(), 76);
    assert!(
        written.values().all(|count| *count == 1),
        "no shot was taken twice"
    );
    assert_eq!(count_pngs(store.dir()), 76);

    let missing = finished
        .pages
        .iter()
        .find(|page| page.url.ends_with("/missing"))
        .expect("the 404 page");
    assert_eq!(missing.status, Some(404));
    assert!(
        store.dir().join(&missing.slug).join("mobile.png").is_file(),
        "captured as well"
    );

    let run_json: Value =
        serde_json::from_slice(&std::fs::read(store.dir().join("run.json")).expect("run.json"))
            .expect("JSON");
    let html = SitemapCapture
        .call_tool(
            &mut MemoryHost::new(),
            "gallery",
            &json!({ "run": run_json }).to_string(),
        )
        .expect("renders");
    let html: Value = serde_json::from_str(&html).expect("JSON");
    let html = html["html"].as_str().expect("html");
    store
        .write_file(GALLERY_FILE, html.as_bytes())
        .expect("gallery");
    assert!(store.dir().join(GALLERY_FILE).is_file());
    let errors = html
        .split("<section id=\"errors\">")
        .nth(1)
        .expect("an error list");
    assert!(errors.contains("HTTP 404"));
    assert!(errors.contains(&format!("{local}/missing")));
    assert_eq!(
        html.matches("<img src=").count(),
        76 * 2,
        "contact sheet and large view"
    );
}
