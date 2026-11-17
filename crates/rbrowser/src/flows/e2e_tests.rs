//! RB-040-07 end to end: the fixture flows of `tests/fixtures/project` run through the sidecar
//! (`node sidecar/src/main.ts`, its flow worker under Node's permission model, the pinned
//! playwright-core and @playwright/test) and one internal CDP proxy per view in front of the
//! scripted browser. The real CEF counterpart is the manual acceptance of the job file.
//!
//! Ignored by default - it needs `node` and `sidecar/node_modules` (`npm ci`);
//! `scripts/check.sh --rust` runs it in the stage `playwright-e2e`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rb_cdp::TargetFilter;
use rb_config::trust::{TrustStore, trust_state, update_trust_store};
use rb_engine::{EngineKind, ViewId};
use rb_mcp::testing::view;
use rb_process::NoConsoleWindow as _;
use rb_sidecar::RpcClient;
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, watch};

use super::endpoints;
use super::plan::{FIREFOX_REASON, Runnable};
use super::result::ViewStatus;
use super::run::{Endpoint, FlowDeps, RunRequest, Setup, execute};
use super::*;
use crate::cdp::CdpRouter;
use crate::cdp::fake::FakeBrowser;
use crate::cdp_proxy::Hooks;

const TARGETS: &[(&str, &str)] = &[
    ("SHELL", crate::shell::SHELL_URL),
    ("V1", "https://a.test/"),
    ("V2", "https://a.test/"),
    ("V3", "https://a.test/"),
    ("V4", "https://a.test/"),
];

fn sidecar_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sidecar")
}

/// The proxies over the scripted browser and the sidecar run from its sources.
struct Deps {
    router: Arc<CdpRouter>,
}

#[async_trait]
impl FlowDeps for Deps {
    fn open_endpoints(
        &self,
        run: &[Runnable],
        targets: &[String],
    ) -> Result<Vec<Endpoint>, FlowError> {
        let hooks = |filter: &'static dyn TargetFilter| Hooks {
            scale: Arc::new(|_: &str| None),
            on_end: Arc::new(|_| {}),
            filter,
        };
        endpoints::open(&Handle::current(), &self.router, &hooks, run, targets)
    }

    fn pause_sync(&self, _views: &[ViewId]) -> Box<dyn Send + Sync> {
        Box::new(())
    }

    async fn run_flow(
        &self,
        params: Value,
        timeout: Duration,
        progress: mpsc::UnboundedSender<Value>,
    ) -> Result<Value, FlowError> {
        let mut node = tokio::process::Command::new("node");
        node.arg("src/main.ts")
            .current_dir(sidecar_dir())
            .env_remove("NODE_OPTIONS")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .no_console_window();
        let mut child = node
            .spawn()
            .map_err(|e| FlowError::Sidecar(e.to_string()))?;
        let (Some(writer), Some(reader)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(FlowError::Sidecar("no pipes".to_owned()));
        };
        let client = RpcClient::start(reader, writer);
        client
            .request_with_progress("flows.run", params, Some(timeout), progress)
            .await
            .map_err(|e| FlowError::Sidecar(e.to_string()))
    }
}

/// The fixture project in a temporary folder, trusted as it is.
fn trusted_project() -> (tempfile::TempDir, tempfile::TempDir) {
    let project = tempfile::tempdir().expect("a project");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/project/.rbrowser");
    let flows = project.path().join(".rbrowser/flows");
    std::fs::create_dir_all(&flows).expect("folders");
    std::fs::copy(
        fixture.join("config.json"),
        project.path().join(".rbrowser/config.json"),
    )
    .expect("config");
    for flow in ["smoke.ts", "check.ts"] {
        std::fs::copy(fixture.join("flows").join(flow), flows.join(flow)).expect("flow");
    }
    let settings = tempfile::tempdir().expect("settings");
    let hash = trust_state(project.path(), &TrustStore::default())
        .expect("a state")
        .hash;
    update_trust_store(
        &settings.path().join(rb_config::trust::TRUST_FILE),
        |store| {
            store.grant(project.path(), &hash, "2026-11-11T10:00:00Z");
        },
    )
    .expect("trusts");
    (project, settings)
}

fn setup(root: &Path, settings: &Path) -> Setup {
    let mut firefox = view(4, "desktop", "https://a.test/");
    firefox.engine = EngineKind::Firefox;
    let views = vec![
        view(1, "mobile", "https://a.test/"),
        view(2, "tablet", "https://a.test/"),
        view(3, "desktop", "https://a.test/"),
        firefox,
    ];
    let targets = (1..=4)
        .map(|id| (ViewId(id), format!("V{id}")))
        .collect::<BTreeMap<_, _>>();
    Setup {
        root: root.to_path_buf(),
        settings: rb_config::FlowSettings::default(),
        views,
        targets,
        env: Some("https://a.test".to_owned()),
        trust_store: Some(settings.join(rb_config::trust::TRUST_FILE)),
        started_at: "2026-11-11T10:15:00Z".to_owned(),
        tmp_root: None,
    }
}

async fn run_fixture(
    name: &str,
    run_id: &str,
) -> (super::result::RunResult, Arc<FakeBrowser>, PathBuf) {
    assert!(
        sidecar_dir().join("node_modules/@playwright/test").is_dir(),
        "run `npm ci` in sidecar/ first (scripts/check.sh --preflight does)"
    );
    let (fake, router) = FakeBrowser::connect();
    fake.auto_attach();
    for (id, url) in TARGETS {
        fake.add_target(id, url);
    }
    let (project, settings) = trusted_project();
    let setup = setup(project.path(), settings.path());
    let request = RunRequest {
        name: name.to_owned(),
        views: None,
        run_id: run_id.to_owned(),
        inline: None,
    };
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _seen) = mpsc::unbounded_channel();
    let result = tokio::time::timeout(
        Duration::from_secs(120),
        execute(&setup, &request, &Deps { router }, cancelled, progress),
    )
    .await
    .expect("the run ends in time")
    .expect("runs");
    let root = project.keep();
    drop(settings);
    (result, fake, root)
}

#[tokio::test]
#[ignore = "needs node and sidecar/node_modules; scripts/check.sh runs it as the stage playwright-e2e"]
async fn a_flow_runs_in_each_chromium_view_through_its_own_endpoint_and_firefox_is_skipped() {
    let (result, fake, root) = run_fixture("smoke", "e2e-smoke").await;
    let statuses: Vec<(u32, ViewStatus)> =
        result.views.iter().map(|v| (v.view_id, v.status)).collect();
    assert_eq!(
        statuses,
        [
            (1, ViewStatus::Passed),
            (2, ViewStatus::Passed),
            (3, ViewStatus::Passed),
            (4, ViewStatus::Skipped)
        ],
        "{result:#?}"
    );
    assert!(result.passed);
    assert_eq!(result.views[3].reason.as_deref(), Some(FIREFOX_REASON));
    assert_eq!(result.views[0].steps[0].name, "open");
    // Each view navigated its own page, and only it.
    let navigations: BTreeMap<String, String> = fake
        .sent("Page.navigate")
        .iter()
        .map(|m| {
            (
                fake.target_of(m["sessionId"].as_str().unwrap_or_default())
                    .unwrap_or_default(),
                m["params"]["url"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        navigations,
        BTreeMap::from([
            ("V1".to_owned(), "https://a.test/mobile".to_owned()),
            ("V2".to_owned(), "https://a.test/tablet".to_owned()),
            ("V3".to_owned(), "https://a.test/desktop".to_owned()),
        ])
    );
    let written = std::fs::read_to_string(root.join(".rbrowser/out/flows/e2e-smoke/result.json"))
        .expect("result.json");
    let written: Value = serde_json::from_str(&written).expect("JSON");
    assert_eq!(written["views"].as_array().map(Vec::len), Some(4));
    assert!(
        !root.join(".rbrowser/out/flows/e2e-smoke/.tmp").exists(),
        "the temp folder is gone"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
#[ignore = "needs node and sidecar/node_modules; scripts/check.sh runs it as the stage playwright-e2e"]
async fn a_failing_expect_fails_only_its_view_with_step_and_line() {
    let (result, _fake, root) = run_fixture("check", "e2e-check").await;
    assert!(!result.passed);
    let tablet = &result.views[1];
    assert_eq!(tablet.status, ViewStatus::Failed, "{result:#?}");
    let error = tablet.error.as_ref().expect("an error");
    assert_eq!(error.line, Some(3));
    assert_eq!(error.step.as_deref(), Some("check the breakpoint"));
    assert!(error.file.ends_with("check.ts"), "{}", error.file);
    assert_eq!(result.views[0].status, ViewStatus::Passed);
    assert_eq!(result.views[2].status, ViewStatus::Passed);
    assert_eq!(json!(result.views[3].status), json!("skipped"));
    let _ = std::fs::remove_dir_all(root);
}
