use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rb_config::trust::{TrustStore, trust_state, update_trust_store};
use rb_engine::EngineKind;
use rb_mcp::testing::view;
use serde_json::json;

use super::*;
use crate::cdp::lock;

/// Drops set its flag: tells whether a guard was let go.
struct Flag(Arc<AtomicBool>);

impl Drop for Flag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
pub(super) struct Fake {
    pub(super) opened: Mutex<Vec<(Vec<u32>, Vec<String>)>>,
    paused: Mutex<Vec<Vec<u32>>>,
    resumed: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
    pub(super) calls: Mutex<Vec<(Value, Duration)>>,
    /// The answer of `flows.run`; `None` never answers.
    pub(super) answer: Mutex<Option<Result<Value, FlowError>>>,
}

#[async_trait]
impl FlowDeps for Fake {
    fn open_endpoints(
        &self,
        run: &[Runnable],
        targets: &[String],
    ) -> Result<Vec<Endpoint>, FlowError> {
        lock(&self.opened).push((run.iter().map(|r| r.view.id.0).collect(), targets.to_vec()));
        Ok(run
            .iter()
            .map(|r| Endpoint {
                value: json!({ "url": format!("http://127.0.0.1:1/{}", r.target), "headers": {} }),
                guard: Box::new(Flag(self.closed.clone())),
            })
            .collect())
    }

    fn pause_sync(&self, views: &[ViewId]) -> Box<dyn Send + Sync> {
        lock(&self.paused).push(views.iter().map(|v| v.0).collect());
        Box::new(Flag(self.resumed.clone()))
    }

    async fn run_flow(
        &self,
        params: Value,
        timeout: Duration,
        progress: mpsc::UnboundedSender<Value>,
    ) -> Result<Value, FlowError> {
        lock(&self.calls).push((params, timeout));
        let _ = progress.send(json!({ "viewId": 1, "status": "running" }));
        let answer = lock(&self.answer).clone();
        match answer {
            Some(answer) => answer,
            None => std::future::pending().await,
        }
    }
}

pub(super) struct Project {
    pub(super) root: tempfile::TempDir,
    settings: tempfile::TempDir,
}

impl Project {
    pub(super) fn new(flow: &str) -> Self {
        let root = tempfile::tempdir().expect("a project");
        let rbrowser = root.path().join(".rbrowser");
        std::fs::create_dir_all(rbrowser.join("flows")).expect("folders");
        std::fs::write(
            rbrowser.join("config.json"),
            r#"{ "version": 1, "breakpoints": [{ "id": "mobile", "width": 375, "height": 812 }] }"#,
        )
        .expect("config");
        std::fs::write(rbrowser.join("flows/login.ts"), flow).expect("flow");
        Self {
            root,
            settings: tempfile::tempdir().expect("settings"),
        }
    }

    fn store(&self) -> PathBuf {
        self.settings.path().join(rb_config::trust::TRUST_FILE)
    }

    pub(super) fn trust(&self) {
        let root = self.root.path();
        let hash = trust_state(root, &TrustStore::default())
            .expect("a state")
            .hash;
        update_trust_store(&self.store(), |store| {
            store.grant(root, &hash, "2026-11-11T10:00:00Z");
        })
        .expect("writes the store");
    }

    pub(super) fn setup(&self) -> Setup {
        let mut firefox = view(3, "desktop", "https://a.test/");
        firefox.engine = EngineKind::Firefox;
        Setup {
            root: self.root.path().to_path_buf(),
            settings: FlowSettings::default(),
            views: vec![
                view(1, "mobile", "https://a.test/"),
                view(2, "tablet", "https://a.test/"),
                firefox,
            ],
            targets: BTreeMap::from([
                (ViewId(1), "T1".to_owned()),
                (ViewId(2), "T2".to_owned()),
                (ViewId(3), "T3".to_owned()),
            ]),
            hidden: std::collections::BTreeSet::new(),
            env: Some("https://a.test".to_owned()),
            trust_store: Some(self.store()),
            started_at: "2026-11-11T10:15:00Z".to_owned(),
            tmp_root: Some(PathBuf::from("/local/rBrowser").join(FLOWS_TMP_DIR)),
        }
    }
}

fn request() -> RunRequest {
    RunRequest {
        name: "login".to_owned(),
        views: None,
        run_id: "20261111-101500-a1b2c3".to_owned(),
        inline: None,
    }
}

/// A stored recording's flow (RB-090-66): not in the flows folder, its breakpoint `mobile`.
pub(super) fn played() -> RunRequest {
    RunRequest {
        name: "checkout".to_owned(),
        inline: Some(InlineFlow {
            source: "await page.goto(env);\n".to_owned(),
            file: ".rbrowser/out/recordings/checkout.json".to_owned(),
            views: vec!["mobile".to_owned()],
            pause_ms: 4_000,
        }),
        ..request()
    }
}

pub(super) fn passed(id: u32) -> Value {
    json!({ "viewId": id, "label": format!("V{id}"), "breakpoint": null, "engine": "chromium",
            "status": "passed", "durationMs": 5, "steps": [], "error": null, "screenshot": null })
}

async fn execute_with(
    project: &Project,
    setup: &Setup,
    fake: &Fake,
) -> (Result<RunResult, FlowError>, Vec<Value>) {
    let _ = project;
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, mut received) = mpsc::unbounded_channel();
    let result = execute(setup, &request(), fake, cancelled, progress).await;
    let mut seen = Vec::new();
    while let Ok(value) = received.try_recv() {
        seen.push(value);
    }
    (result, seen)
}

#[tokio::test]
async fn an_untrusted_project_runs_nothing_and_reaches_no_view() {
    let project = Project::new("await page.goto(env);\n");
    let fake = Fake::default();
    let (result, _) = execute_with(&project, &project.setup(), &fake).await;
    assert_eq!(result, Err(FlowError::Untrusted));
    assert!(lock(&fake.opened).is_empty());
    assert!(lock(&fake.calls).is_empty());
}

#[tokio::test]
async fn a_trusted_run_reaches_each_chromium_view_and_skips_firefox() {
    let project = Project::new("// @timeout 5000\nawait page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    *lock(&fake.answer) = Some(Ok(json!({ "views": [passed(2), passed(1)] })));
    let (result, progress) = execute_with(&project, &project.setup(), &fake).await;
    let result = result.expect("runs");
    assert!(result.passed);
    let statuses: Vec<(u32, ViewStatus)> =
        result.views.iter().map(|v| (v.view_id, v.status)).collect();
    assert_eq!(
        statuses,
        [
            (1, ViewStatus::Passed),
            (2, ViewStatus::Passed),
            (3, ViewStatus::Skipped)
        ]
    );
    assert_eq!(lock(&fake.opened)[0].0, [1, 2]);
    assert_eq!(lock(&fake.opened)[0].1, ["T1", "T2", "T3"]);
    let (params, timeout) = lock(&fake.calls)[0].clone();
    assert_eq!(params["timeoutMs"], json!(5000));
    // One action's wait goes with it; the sidecar holds it to the flow's (RB-090-76).
    assert_eq!(params["actionTimeoutMs"], json!(10_000));
    assert_eq!(params["env"], json!("https://a.test"));
    // The worker's folder is the app's local one, never the project's (RB-090-72).
    let tmp = PathBuf::from("/local/rBrowser")
        .join(FLOWS_TMP_DIR)
        .join("20261111-101500-a1b2c3");
    assert_eq!(params["tmpDir"], json!(tmp.to_string_lossy()));
    assert_eq!(
        params["flow"]["source"],
        json!("// @timeout 5000\nawait page.goto(env);\n")
    );
    assert_eq!(
        params["views"][1]["endpoint"]["url"],
        json!("http://127.0.0.1:1/T2")
    );
    assert_eq!(
        timeout,
        Duration::from_millis(5000) + VIEW_OVERHEAD + RUN_GRACE
    );
    // Every view is named at the start - the run's waiting, the Firefox one skipped - so the
    // panel shows the whole run before the sidecar reports (RB-090-71).
    assert_eq!(
        progress,
        [
            json!({ "viewId": 1, "status": "queued" }),
            json!({ "viewId": 2, "status": "queued" }),
            json!({ "viewId": 3, "status": "skipped" }),
            json!({ "viewId": 1, "status": "running" }),
        ]
    );
    // Sync was paused for the run and is back; the endpoints are closed.
    assert_eq!(*lock(&fake.paused), [vec![1, 2]]);
    assert!(fake.resumed.load(Ordering::SeqCst));
    assert!(fake.closed.load(Ordering::SeqCst));
    let written = super::super::result::read(&run_dir(project.root.path(), &result.run_id));
    assert_eq!(written, Some(result));
}

#[tokio::test]
async fn sync_stays_on_when_the_setting_says_so() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    *lock(&fake.answer) = Some(Ok(json!({ "views": [passed(1), passed(2)] })));
    let mut setup = project.setup();
    setup.settings.sync_during_run = true;
    let (result, _) = execute_with(&project, &setup, &fake).await;
    assert!(result.is_ok());
    assert!(lock(&fake.paused).is_empty());
}

#[tokio::test]
async fn a_failing_sidecar_fails_the_views_and_the_result_is_still_written() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    *lock(&fake.answer) = Some(Err(FlowError::Sidecar("node is missing".to_owned())));
    let (result, _) = execute_with(&project, &project.setup(), &fake).await;
    let result = result.expect("a result");
    assert!(!result.passed);
    assert_eq!(result.counts(), (0, 2, 1));
    let error = result.views[0].error.clone().expect("an error");
    assert!(error.message.contains("node is missing"));
    assert!(error.file.ends_with("login.ts"));
}

#[tokio::test]
async fn a_cancelled_run_ends_and_fails_its_views() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    let (cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let setup = project.setup();
    let asked = request();
    let run = execute(&setup, &asked, &fake, cancelled, progress);
    let (result, ()) = tokio::join!(run, async {
        tokio::task::yield_now().await;
        let _ = cancel.send(true);
    });
    let result = result.expect("a result");
    assert_eq!(
        result.views[0].error.as_ref().map(|e| e.message.as_str()),
        Some(CANCELLED)
    );
    assert!(fake.closed.load(Ordering::SeqCst));
    assert!(
        fake.resumed.load(Ordering::SeqCst),
        "sync is back after a cancel"
    );
}

#[tokio::test]
async fn a_run_dropped_midway_gives_sync_back() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let setup = project.setup();
    let asked = request();
    let run = execute(&setup, &asked, &fake, cancelled, progress);
    // The task of the run is aborted while the sidecar works.
    let ended = tokio::time::timeout(Duration::from_millis(50), run).await;
    assert!(ended.is_err());
    assert_eq!(*lock(&fake.paused), [vec![1, 2]]);
    assert!(fake.resumed.load(Ordering::SeqCst));
    assert!(fake.closed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn a_missing_flow_names_the_known_ones() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let mut asked = request();
    asked.name = "cart".to_owned();
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let result = execute(
        &project.setup(),
        &asked,
        &Fake::default(),
        cancelled,
        progress,
    )
    .await;
    assert_eq!(
        result,
        Err(FlowError::NotFound {
            name: "cart".to_owned(),
            known: vec!["login".to_owned()],
        })
    );
}

#[tokio::test]
async fn a_run_that_fails_before_its_views_still_writes_its_result() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let mut asked = request();
    asked.name = "cart".to_owned();
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let setup = project.setup();
    let outcome = execute_recorded(&setup, &asked, &Fake::default(), cancelled, progress).await;
    assert!(matches!(outcome, Err(FlowError::NotFound { .. })));
    // The CLI waiting for this run id reads at once that it failed, and why.
    let written = super::super::result::read(&run_dir(project.root.path(), &asked.run_id))
        .expect("a result.json");
    assert!(!written.passed);
    assert!(written.views.is_empty());
    assert_eq!(written.flow, "cart");
    assert!(
        written
            .error
            .as_deref()
            .is_some_and(|error| error.contains("cart")),
        "{written:?}"
    );
}

#[tokio::test]
async fn a_run_with_no_valid_id_writes_nothing() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let mut asked = request();
    asked.run_id = "../escape".to_owned();
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let setup = project.setup();
    let outcome = execute_recorded(&setup, &asked, &Fake::default(), cancelled, progress).await;
    assert!(outcome.is_err());
    assert!(!project.root.path().join(".rbrowser/out").exists());
}

#[tokio::test]
async fn a_recording_runs_from_memory_in_the_views_of_its_breakpoints() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    *lock(&fake.answer) = Some(Ok(json!({ "views": [passed(1)] })));
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let asked = played();
    let result = execute(&project.setup(), &asked, &fake, cancelled, progress)
        .await
        .expect("runs");
    assert!(result.passed);
    assert_eq!(result.flow, "checkout");
    assert_eq!(result.file, ".rbrowser/out/recordings/checkout.json");
    // Only the view of the recording's breakpoint runs; nothing else is planned.
    assert_eq!(lock(&fake.opened)[0].0, [1]);
    let (params, _) = lock(&fake.calls)[0].clone();
    assert_eq!(params["flow"]["name"], json!("checkout"));
    // RB-090-84: the recorded pauses on top of the flow's timeout (30 s by default).
    assert_eq!(params["timeoutMs"], json!(34_000));
    assert_eq!(params["flow"]["source"], json!("await page.goto(env);\n"));
    assert_eq!(
        params["flow"]["file"],
        json!(".rbrowser/out/recordings/checkout.json")
    );
    // Nothing new in the flows folder.
    let flows: Vec<String> = std::fs::read_dir(project.root.path().join(".rbrowser/flows"))
        .expect("the folder")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(flows, ["login.ts"]);
}

#[tokio::test]
async fn views_asked_for_win_over_the_recordings_breakpoints() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = Fake::default();
    *lock(&fake.answer) = Some(Ok(json!({ "views": [passed(2)] })));
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let mut asked = played();
    asked.views = Some(vec![ViewId(2)]);
    let result = execute(&project.setup(), &asked, &fake, cancelled, progress).await;
    assert!(result.is_ok());
    assert_eq!(lock(&fake.opened)[0].0, [2]);
}

#[tokio::test]
async fn a_recording_runs_only_in_a_trusted_workspace() {
    let project = Project::new("await page.goto(env);\n");
    let fake = Fake::default();
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let result = execute(&project.setup(), &played(), &fake, cancelled, progress).await;
    assert_eq!(result, Err(FlowError::Untrusted));
    assert!(lock(&fake.opened).is_empty());
    assert!(lock(&fake.calls).is_empty());
}

#[tokio::test]
async fn a_recording_that_fails_before_its_views_names_its_own_file() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let mut asked = played();
    if let Some(inline) = asked.inline.as_mut() {
        inline.views = vec!["watch".to_owned()];
    }
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let setup = project.setup();
    let outcome = execute_recorded(&setup, &asked, &Fake::default(), cancelled, progress).await;
    let Err(FlowError::NoViews(why)) = &outcome else {
        panic!("no views: {outcome:?}");
    };
    assert!(why.contains("watch") && why.contains("recording"), "{why}");
    let written = super::super::result::read(&run_dir(project.root.path(), &asked.run_id))
        .expect("a result.json");
    assert_eq!(written.file, ".rbrowser/out/recordings/checkout.json");
    assert_eq!(written.flow, "checkout");
}

#[test]
fn errors_name_the_linux_path_of_a_wsl_project() {
    assert_eq!(
        display_path(Path::new(
            r"\\wsl.localhost\Ubuntu\home\u\site\.rbrowser\flows\login.ts"
        )),
        "/home/u/site/.rbrowser/flows/login.ts"
    );
    assert_eq!(
        display_path(Path::new("/srv/site/.rbrowser/flows/login.ts")),
        "/srv/site/.rbrowser/flows/login.ts"
    );
}

#[test]
fn the_budget_is_one_timeout_and_its_overhead_per_round_of_parallel_views() {
    let round = Duration::from_secs(1) + VIEW_OVERHEAD;
    assert_eq!(budget(4, 0, 1000), round + RUN_GRACE);
    assert_eq!(budget(4, 1, 1000), round * 4 + RUN_GRACE);
    assert_eq!(budget(5, 2, 1000), round * 3 + RUN_GRACE);
}

#[test]
fn the_sidecar_stops_its_worker_before_rbrowser_gives_up_on_it() {
    // sidecar/src/modules/flows: VIEW_OVERHEAD_MS (result.ts) and GRACE_MS (real.ts).
    assert_eq!(VIEW_OVERHEAD, Duration::from_secs(10));
    assert!(RUN_GRACE > Duration::from_secs(15));
}
